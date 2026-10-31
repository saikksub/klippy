# Klippy

A clipboard history for macOS that lives in the menu bar and saves only what you ask it to.

Most clipboard managers record everything you copy, including passwords, one-time codes and
private messages. Klippy records nothing by default. To save something, **copy it three times
in quick succession** (⌘C ⌘C ⌘C within 2 seconds). Ordinary single copies never reach the
history.

## What it does

- **Saves on request.** Copying something 3 times within 2 s adds it to the history. You can
  set the trigger from 1 to 4 copies in Settings. The time window is 0.5 s plus 0.5 s per copy.
- **Keeps the formatting.** It saves every format the source app provided (HTML, RTF, plain
  text…), so pasting a saved item keeps its styles.
- **Menu bar access.** The menu bar menu lists your 20 most recent items; click one to copy it
  again. The History window shows the full history, with search and a preview of each item.
- **Export.** It exports an item as PDF, HTML or Markdown, or sends it straight to Downloads
  with Quick Export.
- **Deletes old items automatically.** Items not copied again within the retention period
  (30 days by default; you can choose from 1 minute to 365 days) are deleted. The history
  holds at most 1,000 items. *Clear History…* deletes everything.
- **Launch at login.** This is on by default and can be turned off in Settings or in System
  Settings → General → Login Items.

## How it keeps your data safe

### Your history is encrypted and locked behind Touch ID
- The history is a SQLite database encrypted with **SQLCipher (AES-256)**, stored at
  `~/Library/Application Support/Klippy/history.sqlite` in a folder only your user can read
  (`0700`). Without the key, the file is unreadable, including its header.
- The 256-bit key is generated at random on first launch and kept in your **login
  Keychain**. The Keychain item is marked *this device only, when unlocked*, so it isn't
  synced or included in backups.
- Klippy asks for **Touch ID (or your login password)** before it takes the key out of the
  Keychain. If you cancel, the history stays locked and nothing is read or saved.
- Deleted items are overwritten on disk (`secure_delete`), and *Clear History* also compacts
  the file.

### It reads the clipboard only when you ask it to save
- In the background, Klippy only checks the clipboard's **change counter**. That is a single
  number, not your clipboard's contents, so ordinary copies are never read.
- It reads the contents only after the save trigger (a triple copy).
- It skips anything the source app marks as private or temporary, following
  [nspasteboard.org](https://nspasteboard.org). Password managers (1Password, Bitwarden…) use
  these markers, so passwords copied from them are never saved, even with a triple copy.
- Items larger than 10 MB are skipped.

### Copied web content can't attack you
HTML copied from a web page could have been written to run code. Klippy handles it in four
layers:

1. **Rejecting attacks.** HTML that contains scripts, event handlers (`onclick`…),
   `javascript:` links, frames, plug-ins, `<base>` or a meta refresh is **not saved**. It
   appears under History → *Failed* with the reasons. That list is kept in memory only and
   keeps just the plain text. Klippy also sends you a notification.
2. **Cleaning.** For the preview, the HTML is rebuilt from an allowlist using
   [ammonia](https://github.com/rust-ammonia/ammonia). Only formatting tags and harmless
   inline CSS are kept; nothing that can load a URL survives.
3. **Safe images.** Images are decoded with memory-safe Rust decoders, with size limits that
   stop decompression bombs, and then re-encoded. This strips metadata such as EXIF and GPS,
   and anything hidden in the file. SVGs are converted to PNG without running scripts.
4. **A locked-down viewer.** The preview has JavaScript turned off. A content blocker and a
   Content-Security-Policy block all network access, every link and redirect is cancelled,
   and nothing is stored. It runs in WebKit's separately sandboxed process.

Exports are built from the cleaned copy, so they contain no scripts and load nothing from the
network.

### Network access
Klippy has **no accounts, analytics, telemetry or update checks**. The only network requests
it makes are downloads of the **images in HTML you chose to save**, so the preview can show
them. Those downloads:
- use HTTPS only,
- send no cookies, cache or saved logins,
- are limited to 20 images per item, 5 MB per image and a 10 s timeout.

The image host can see that the image was downloaded, just as it would if you opened the page.

### Limitations
- **Pasting returns the original.** When you copy a saved item again, Klippy puts back the
  formats the source app provided, unchanged, so it pastes exactly as it was copied. The
  cleaned version is used only for the preview and for exports.
- **Klippy isn't sandboxed or notarized.** You build it yourself, and it is signed with a
  local certificate (`make dev-cert`), not an Apple Developer ID. The source is in this repo
  for you to check.
- **Touch ID is enforced by Klippy, not by macOS.** Another app trying to read the Keychain
  item still triggers the usual macOS Keychain prompt.

## Permissions

| Permission | Why | Required? |
| --- | --- | --- |
| **Paste from Other Apps** (Privacy & Security), set to *Always Allow* | Reading the clipboard after a triple copy without a prompt each time | Yes. If it isn't set, Klippy shows a setup page |
| **Touch ID / login password** | Unlocking the history's key | Yes, once per launch |
| **Keychain access**: choose *Always Allow* | Storing the history key | Yes, the first time |
| **Notifications** | Telling you when a copy is rejected or an export is saved | Optional |
| **Login Items** | Launch at login | Optional |

Klippy doesn't need Accessibility, Input Monitoring, Full Disk Access or Screen Recording.

## Install for daily use

```sh
./scripts/make-dev-cert.sh   # once: a local signing certificate (asks for your password)
./scripts/install.sh         # build, install to /Applications and launch; run again to update
./scripts/build-dmg.sh       # or: build apple/build.noindex/Klippy-<version>.dmg
```

The scripts check for the tools listed under [Prerequisites](#prerequisites) and tell you how to
install any that are missing.

Run Klippy from `/Applications`, not `apple/build.noindex/`. `make app` and `make clean` delete
`apple/build.noindex`, which would break launch at login.

After each new build, macOS asks **twice** for access to the "Klippy history encryption key"
the first time Klippy runs. Enter your password and choose **Always Allow** both times. It
happens because Keychain ties the key to the build's code hash: a self-signed certificate has no
Apple Team ID, so a signature that stays the same across builds would need an Apple Developer ID.

## Development

Klippy is a [Crux](https://github.com/redbadger/crux) app: the behaviour lives in a Rust core
(`shared/`), and a SwiftUI shell (`apple/`) runs it as a native macOS desktop app.

```
shared/            Rust core: Event, Model, ViewModel, Effect, update/view (src/app.rs)
  src/store.rs     encrypted history (SQLCipher)
  src/clean.rs     threat detection, HTML sanitising, image cleaning
  src/export.rs    PDF / HTML / Markdown export
  src/ffi.rs       CoreFfi, the bytes-in/bytes-out interface exported via BoltFFI
  src/bin/codegen  generates Swift types for Event/ViewModel/Effect
apple/             SwiftUI macOS shell (Swift Package, no Xcode project)
  Sources/Klippy   App entry point, Core (effect loop), ContentView
  generated/       build output of `make generate`, not checked in
```

## Prerequisites

- Rust (stable) with the `aarch64-apple-darwin` target
- BoltFFI CLI, matching the `boltffi` crate version:
  `cargo install boltffi_cli --version '=0.30.1' --locked`
- Swift 5.9+ (Xcode Command Line Tools are enough; full Xcode is optional)

## Commands

| Command | What it does |
| --- | --- |
| `make run` | generate, build, bundle `apple/build.noindex/Klippy.app` and open it |
| `make test` | Rust core tests + Swift shell tests against the real core |
| `make app CONFIG=release` | optimized `.app` bundle |
| `make dmg` | release build packed into `apple/build.noindex/Klippy-<version>.dmg` |
| `make generate` | only regenerate Swift types and the Rust static library |
| `make open` | open the Swift package in Xcode (if installed) |
| `make clean` | remove all build output |

Re-run `make generate` (any of `run`/`app`/`test` does it for you) after changing
types in `shared/src/app.rs`.

## How it fits together

1. The shell sends an `Event` to the core (`Core.update` → `CoreFfi.update`), serialized with bincode.
2. The core runs `update`, changes the `Model` and returns the effects it wants performed.
3. The shell carries them out; for `Render` it fetches the new `ViewModel` and SwiftUI re-renders.

To add a side effect (HTTP, key-value storage, timers…), add a variant to `Effect` in
`shared/src/app.rs` (e.g. with `crux_http`), run `make generate`, and handle the new case
in `Core.processEffect` in `apple/Sources/Klippy/Core.swift`.

## Notes

- `boltffi pack apple` (used by the upstream Crux examples) builds iOS targets and an
  xcframework, which requires full Xcode. This project only needs macOS, so `make package`
  builds `libshared.a` with cargo and generates bindings with `boltffi generate swift` instead.
- The build is for the host architecture (Apple silicon). For a universal binary, build
  `x86_64-apple-darwin` too and combine the libraries with `lipo`.
