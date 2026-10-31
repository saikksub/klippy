//! Persistent clipboard history in SQLite, encrypted with SQLCipher
//! (AES-256, via Apple's CommonCrypto). The key comes from the shell, which
//! keeps it in the Keychain behind Touch ID.
//!
//! The list only ever needs `id` and a short `preview`, so those live in the
//! small `clips` table and are all the app keeps in memory. The heavy formats
//! (HTML, RTF, …) sit in `formats` and are read only when an item is opened or
//! copied again.

use std::{path::Path, sync::Mutex};

use rusqlite::{Connection, OptionalExtension, params};

use crate::{Appearance, ClipItem, ClipboardContent, ClipboardFormat, appearance};

/// A ceiling on top of the age-based clean-up, so a very long retention can't
/// grow the database without bound.
pub const MAX_ITEMS: usize = 1000;

/// Longest preview stored for the list; the full text stays in `text`.
const PREVIEW_CHARS: usize = 300;

/// 2 added `clips.appearance`.
const SCHEMA_VERSION: i32 = 2;

/// SQLCipher's raw key size: 256 bits.
pub const KEY_BYTES: usize = 32;

pub struct Store {
    // `Connection` isn't `Sync`; the core needs the model to be.
    conn: Mutex<Connection>,
}

impl Store {
    /// Opens (or creates) the encrypted database. Fails if `key` is wrong.
    pub fn open(path: impl AsRef<Path>, key: &[u8; KEY_BYTES]) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        // A raw key (`x'…'`) skips SQLCipher's password stretching; it must be
        // set before anything else touches the file.
        let hex: String = key.iter().map(|byte| format!("{byte:02x}")).collect();
        conn.execute_batch(&format!("PRAGMA key = \"x'{hex}'\";"))?;
        Self::new(conn)
    }

    /// For tests, and as a fallback when the file can't be opened.
    pub fn in_memory() -> Self {
        Self::new(Connection::open_in_memory().expect("in-memory SQLite"))
            .expect("in-memory schema")
    }

    fn new(mut conn: Connection) -> rusqlite::Result<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA secure_delete = ON;",
        )?;
        let version: i32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version < 1 {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS clips (
                     id          INTEGER PRIMARY KEY AUTOINCREMENT, -- never reused
                     seq         INTEGER NOT NULL,  -- list order, higher = newer
                     copied_at   INTEGER NOT NULL,  -- unix ms of the last copy
                     text        TEXT    NOT NULL,
                     search_text TEXT    NOT NULL,  -- lowercased, for search
                     preview     TEXT    NOT NULL,
                     text_hash   INTEGER NOT NULL   -- finds duplicates cheaply
                 );
                 CREATE INDEX IF NOT EXISTS clips_seq ON clips(seq DESC);
                 CREATE INDEX IF NOT EXISTS clips_copied_at ON clips(copied_at);
                 CREATE INDEX IF NOT EXISTS clips_hash ON clips(text_hash);
                 CREATE TABLE IF NOT EXISTS formats (
                     clip_id INTEGER NOT NULL REFERENCES clips(id) ON DELETE CASCADE,
                     kind    TEXT    NOT NULL,
                     data    BLOB    NOT NULL
                 );
                 CREATE INDEX IF NOT EXISTS formats_clip ON formats(clip_id);
                 CREATE TABLE IF NOT EXISTS settings (
                     key   TEXT PRIMARY KEY,
                     value TEXT NOT NULL
                 );
                 PRAGMA user_version = 1;",
            )?;
        }
        if version < 2 {
            // Items saved before then are decided now, from what was stored.
            let tx = conn.transaction()?;
            tx.execute_batch(
                "ALTER TABLE clips ADD COLUMN appearance TEXT NOT NULL DEFAULT 'system';",
            )?;
            let ids = tx
                .prepare("SELECT id FROM clips")?
                .query_map([], |row| row.get::<_, i64>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            for id in ids {
                if let Some(content) = content_in(&tx, id)? {
                    tx.execute(
                        "UPDATE clips SET appearance = ?1 WHERE id = ?2",
                        params![appearance::detect(&content).name(), id],
                    )?;
                }
            }
            tx.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION};"))?;
            tx.commit()?;
        }
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        // A panic mid-query can't leave SQLite inconsistent, so carry on.
        self.conn
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Adds to the top of the history. Text already there moves to the top
    /// and takes the new formats. Whether it's light or dark is decided here,
    /// once, from those formats.
    pub fn add(&self, content: &ClipboardContent, now_ms: u64) -> rusqlite::Result<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        let hash = text_hash(&content.text);
        let appearance = appearance::detect(content).name();
        let seq: i64 = tx.query_row("SELECT COALESCE(MAX(seq), 0) + 1 FROM clips", [], |row| {
            row.get(0)
        })?;
        let existing: Option<i64> = tx
            .query_row(
                "SELECT id FROM clips WHERE text_hash = ?1 AND text = ?2",
                params![hash, content.text],
                |row| row.get(0),
            )
            .optional()?;
        let id = if let Some(id) = existing {
            tx.execute(
                "UPDATE clips SET seq = ?1, copied_at = ?2, appearance = ?3 WHERE id = ?4",
                params![seq, to_sql_ms(now_ms), appearance, id],
            )?;
            tx.execute("DELETE FROM formats WHERE clip_id = ?1", [id])?;
            id
        } else {
            tx.execute(
                "INSERT INTO clips
                     (seq, copied_at, text, search_text, preview, text_hash, appearance)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    seq,
                    to_sql_ms(now_ms),
                    content.text,
                    content.text.to_lowercase(),
                    preview(&content.text),
                    hash,
                    appearance
                ],
            )?;
            tx.last_insert_rowid()
        };
        {
            let mut insert =
                tx.prepare("INSERT INTO formats (clip_id, kind, data) VALUES (?1, ?2, ?3)")?;
            for format in &content.formats {
                insert.execute(params![id, format.kind, format.data])?;
            }
        }
        tx.execute(
            "DELETE FROM clips WHERE id IN
                 (SELECT id FROM clips ORDER BY seq DESC LIMIT -1 OFFSET ?1)",
            [MAX_ITEMS as i64],
        )?;
        tx.commit()
    }

    /// Items whose text contains `search` (ignoring case), newest first.
    pub fn list(&self, search: &str) -> rusqlite::Result<Vec<ClipItem>> {
        self.query_items(search, -1)
    }

    /// The `limit` newest items.
    pub fn recent(&self, limit: usize) -> rusqlite::Result<Vec<ClipItem>> {
        self.query_items("", i64::try_from(limit).unwrap_or(i64::MAX))
    }

    /// `limit` -1 means no limit.
    fn query_items(&self, search: &str, limit: i64) -> rusqlite::Result<Vec<ClipItem>> {
        let search = search.trim().to_lowercase();
        let conn = self.conn();
        let mut query = conn.prepare_cached(
            "SELECT id, preview FROM clips
             WHERE ?1 = '' OR instr(search_text, ?1) > 0
             ORDER BY seq DESC
             LIMIT ?2",
        )?;
        query
            .query_map(params![search, limit], |row| {
                Ok(ClipItem {
                    id: from_sql_id(row.get(0)?),
                    preview: row.get(1)?,
                })
            })?
            .collect()
    }

    /// The full text and formats of one item.
    pub fn content(&self, id: u64) -> rusqlite::Result<Option<ClipboardContent>> {
        content_in(&self.conn(), to_sql_id(id))
    }

    /// Whether the item was made for a light or a dark page.
    pub fn appearance(&self, id: u64) -> rusqlite::Result<Option<Appearance>> {
        let name = self
            .conn()
            .query_row(
                "SELECT appearance FROM clips WHERE id = ?1",
                [to_sql_id(id)],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        Ok(name.map(|name| Appearance::from_name(&name).unwrap_or_default()))
    }

    pub fn contains(&self, id: u64) -> rusqlite::Result<bool> {
        self.conn().query_row(
            "SELECT EXISTS(SELECT 1 FROM clips WHERE id = ?1)",
            [to_sql_id(id)],
            |row| row.get(0),
        )
    }

    /// Deletes items last copied before `cutoff_ms`; returns how many.
    pub fn delete_copied_before(&self, cutoff_ms: u64) -> rusqlite::Result<usize> {
        self.conn().execute(
            "DELETE FROM clips WHERE copied_at < ?1",
            [to_sql_ms(cutoff_ms)],
        )
    }

    /// Deletes every item (settings stay) and compacts the file, so nothing
    /// deleted lingers on disk.
    pub fn clear(&self) -> rusqlite::Result<()> {
        let conn = self.conn();
        conn.execute("DELETE FROM clips", [])?;
        conn.execute_batch("VACUUM;")
    }

    pub fn setting(&self, key: &str) -> rusqlite::Result<Option<String>> {
        self.conn()
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()
    }

    pub fn set_setting(&self, key: &str, value: &str) -> rusqlite::Result<()> {
        self.conn().execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [key, value],
        )?;
        Ok(())
    }
}

fn content_in(conn: &Connection, id: i64) -> rusqlite::Result<Option<ClipboardContent>> {
    let Some(text) = conn
        .query_row("SELECT text FROM clips WHERE id = ?1", [id], |row| {
            row.get::<_, String>(0)
        })
        .optional()?
    else {
        return Ok(None);
    };
    let mut query =
        conn.prepare_cached("SELECT kind, data FROM formats WHERE clip_id = ?1 ORDER BY rowid")?;
    let formats = query
        .query_map([id], |row| {
            Ok(ClipboardFormat {
                kind: row.get(0)?,
                data: row.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(Some(ClipboardContent { text, formats }))
}

/// Collapses all whitespace (including newlines) so the text fits one line,
/// and keeps only the start.
pub fn preview(text: &str) -> String {
    let mut preview = String::new();
    for word in text.split_whitespace() {
        if !preview.is_empty() {
            preview.push(' ');
        }
        preview.push_str(word);
        if preview.chars().count() >= PREVIEW_CHARS {
            return preview.chars().take(PREVIEW_CHARS).collect();
        }
    }
    preview
}

/// FNV-1a: stable across Rust versions, unlike `DefaultHasher`, so stored
/// hashes stay valid.
fn text_hash(text: &str) -> i64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash.cast_signed()
}

// SQLite integers are i64; ids and unix-ms times are positive and well below
// i64::MAX, so these conversions are lossless in practice.
fn to_sql_id(id: u64) -> i64 {
    i64::try_from(id).unwrap_or(i64::MAX)
}

fn from_sql_id(id: i64) -> u64 {
    u64::try_from(id).unwrap_or_default()
}

fn to_sql_ms(ms: u64) -> i64 {
    i64::try_from(ms).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod test {
    use super::*;

    fn content(text: &str) -> ClipboardContent {
        ClipboardContent {
            text: text.into(),
            formats: vec![ClipboardFormat {
                kind: "public.html".into(),
                data: format!("<b>{text}</b>").into_bytes(),
            }],
        }
    }

    fn previews(store: &Store, search: &str) -> Vec<String> {
        store
            .list(search)
            .unwrap()
            .into_iter()
            .map(|item| item.preview)
            .collect()
    }

    fn temp_path(name: &str) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("klippy-{name}-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);
        path
    }

    const KEY: [u8; KEY_BYTES] = [7; KEY_BYTES];

    #[test]
    fn the_file_is_encrypted_and_needs_the_right_key() {
        let path = temp_path("encrypted");
        Store::open(&path, &KEY)
            .unwrap()
            .add(&content("secret words"), 1)
            .unwrap();

        let bytes = std::fs::read(&path).unwrap();
        let plain = |needle: &[u8]| bytes.windows(needle.len()).any(|w| w == needle);
        assert!(!plain(b"SQLite format 3"), "header should be encrypted");
        assert!(!plain(b"secret words"), "text should be encrypted");

        assert!(
            Store::open(&path, &[8; KEY_BYTES]).is_err(),
            "wrong key must fail"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn history_survives_reopening_the_file() {
        let path = temp_path("reopen");
        {
            let store = Store::open(&path, &KEY).unwrap();
            store.add(&content("kept"), 1).unwrap();
            store.set_setting("copy_trigger", "2").unwrap();
        }
        let store = Store::open(&path, &KEY).unwrap();
        assert_eq!(previews(&store, ""), ["kept"]);
        let id = store.list("").unwrap()[0].id;
        assert_eq!(store.content(id).unwrap(), Some(content("kept")));
        assert_eq!(store.setting("copy_trigger").unwrap().as_deref(), Some("2"));
        drop(store);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn re_adding_moves_to_the_top_with_new_formats() {
        let store = Store::in_memory();
        store.add(&content("a"), 1).unwrap();
        store.add(&content("b"), 2).unwrap();
        let mut newer = content("a");
        newer.formats[0].data = b"new".to_vec();
        store.add(&newer, 3).unwrap();

        assert_eq!(previews(&store, ""), ["a", "b"]);
        let id = store.list("").unwrap()[0].id;
        assert_eq!(store.content(id).unwrap(), Some(newer));
    }

    #[test]
    fn search_ignores_case_including_non_ascii() {
        let store = Store::in_memory();
        store.add(&content("Ärger im Büro"), 1).unwrap();
        store.add(&content("other"), 2).unwrap();
        assert_eq!(previews(&store, "  BÜRO "), ["Ärger im Büro"]);
    }

    #[test]
    fn keeps_at_most_max_items() {
        let store = Store::in_memory();
        for i in 0..=MAX_ITEMS {
            store.add(&content(&i.to_string()), i as u64).unwrap();
        }
        let list = previews(&store, "");
        assert_eq!(list.len(), MAX_ITEMS);
        assert_eq!(list.last().unwrap(), "1"); // "0" was the oldest
    }

    #[test]
    fn deletes_items_copied_before_a_cutoff_with_their_formats() {
        let store = Store::in_memory();
        store.add(&content("old"), 100).unwrap();
        store.add(&content("new"), 200).unwrap();
        let old_id = store.list("old").unwrap()[0].id;

        assert_eq!(store.delete_copied_before(150).unwrap(), 1);
        assert_eq!(previews(&store, ""), ["new"]);
        assert_eq!(store.content(old_id).unwrap(), None);
        let orphans: i64 = store
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM formats WHERE clip_id = ?1",
                [to_sql_id(old_id)],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(orphans, 0);
    }

    #[test]
    fn clear_deletes_items_and_their_formats_but_keeps_settings() {
        let store = Store::in_memory();
        store.add(&content("a"), 1).unwrap();
        store.set_setting("copy_trigger", "2").unwrap();
        store.clear().unwrap();
        assert!(store.list("").unwrap().is_empty());
        let formats: i64 = store
            .conn()
            .query_row("SELECT COUNT(*) FROM formats", [], |row| row.get(0))
            .unwrap();
        assert_eq!(formats, 0);
        assert_eq!(store.setting("copy_trigger").unwrap().as_deref(), Some("2"));
    }

    #[test]
    fn decides_the_appearance_when_saving() {
        let store = Store::in_memory();
        let dark = ClipboardContent {
            text: "dark".into(),
            formats: vec![ClipboardFormat {
                kind: "public.html".into(),
                data: br#"<div style="background-color:#1e1e1e;color:#ddd">dark</div>"#.to_vec(),
            }],
        };
        store.add(&dark, 1).unwrap();
        store.add(&content("plain"), 2).unwrap();
        let ids: Vec<u64> = store.list("").unwrap().iter().map(|item| item.id).collect();
        assert_eq!(store.appearance(ids[0]).unwrap(), Some(Appearance::System));
        assert_eq!(store.appearance(ids[1]).unwrap(), Some(Appearance::Dark));
        assert_eq!(store.appearance(999).unwrap(), None);
    }

    #[test]
    fn upgrading_from_version_1_decides_existing_items() {
        let path = temp_path("upgrade");
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE clips (id INTEGER PRIMARY KEY AUTOINCREMENT, seq INTEGER NOT NULL,
                     copied_at INTEGER NOT NULL, text TEXT NOT NULL, search_text TEXT NOT NULL,
                     preview TEXT NOT NULL, text_hash INTEGER NOT NULL);
                 CREATE TABLE formats (clip_id INTEGER NOT NULL REFERENCES clips(id) ON DELETE CASCADE,
                     kind TEXT NOT NULL, data BLOB NOT NULL);
                 CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 INSERT INTO clips VALUES (1, 1, 1, 'x', 'x', 'x', 0);
                 INSERT INTO formats VALUES (1, 'public.html', CAST('<p style=color:#eee>x</p>' AS BLOB));
                 PRAGMA user_version = 1;",
            )
            .unwrap();
        }
        let store = Store::new(Connection::open(&path).unwrap()).unwrap();
        assert_eq!(store.appearance(1).unwrap(), Some(Appearance::Dark));
        drop(store);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn previews_are_short_and_one_line() {
        assert_eq!(preview("fn main() {\n    hi\n}"), "fn main() { hi }");
        assert_eq!(preview(&"word ".repeat(200)).chars().count(), PREVIEW_CHARS);
    }
}
