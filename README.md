# Klippy

A clipboard history for macOS and Windows that lives in the menu bar (or the Windows tray) and
saves only what you ask it to. See [Windows](#windows) for how the Windows version differs.

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

### Where your history is stored
- The history is a SQLite database at `~/Library/Application Support/Klippy/history.sqlite`,
  in a folder only your user can read (`0700`). It is **not encrypted**: any program running
  as you can read it.
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
- **The history file isn't encrypted.** Anything that can read your user's files can read
  your saved copies.

## Permissions

| Permission | Why | Required? |
| --- | --- | --- |
| **Paste from Other Apps** (Privacy & Security), set to *Always Allow* | Reading the clipboard after a triple copy without a prompt each time | Yes. If it isn't set, Klippy shows a setup page |
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

## Windows

Klippy for Windows (10 and 11, x64) runs the same Rust core as the Mac app, so the copy trigger,
cleaning, rejection of malicious HTML, retention and exports behave the same. Save a copy with
**Ctrl+C three times in quick succession**. Klippy lives in the notification area: left-click the
paperclip to open it, or right-click it for History, Settings and Quit.

### Build and run

On a Windows PC with [Rust](https://rustup.rs) (MSVC toolchain, with the Visual Studio C++ Build
Tools):

```powershell
.\scripts\build-windows.ps1   # builds windows\dist\Klippy.exe; run it again to update
```

`Klippy.exe` is a single file with nothing to install alongside it (WebView2 comes with Windows).
Keep it in a folder you won't move, because launch at login starts it from there. Without a
Windows PC, the [Windows workflow](.github/workflows/windows.yml) builds the exe on every push to
`main`; download it from the run's **Klippy-windows-x64** artifact.

The exe isn't code-signed, so SmartScreen warns the first time you run it: choose
*More info → Run anyway*.

### How it differs from the Mac app

| | macOS | Windows |
| --- | --- | --- |
| History file | `~/Library/Application Support/Klippy/` | `%LOCALAPPDATA%\Klippy\history.sqlite` |
| Change detection | Pasteboard change count | Clipboard sequence number (`GetClipboardSequenceNumber`), which doesn't open the clipboard |
| Private copies skipped | nspasteboard.org markers | `ExcludeClipboardContentFromMonitorProcessing`, `Clipboard Viewer Ignore` and `CanIncludeInClipboardHistory = 0`, which password managers set |
| Formats kept | Every pasteboard type | Text, HTML and RTF |
| Preview | Locked-down WKWebView | WebView2 page with the copy in a fully sandboxed `<iframe>` (no scripts, no navigation) under a Content-Security-Policy that blocks all loading |
| PDF export | WebKit printing | WebView2's PDF printer, with JavaScript off |
| Image downloads | URLSession, ephemeral | HTTPS only, no cookies, through Windows' own TLS (Schannel) |
| Launch at login | Login Items | The per-user `Run` key; you can turn it off in Settings → Apps → Startup |
| Notifications | Notification Center | Toast notifications |
| Clipboard permission | *Paste from Other Apps* | None needed |

## Development

Klippy is a [Crux](https://github.com/redbadger/crux) app: the behaviour lives in a Rust core
(`shared/`). A SwiftUI shell (`apple/`) runs it as a native macOS app, and a Rust shell
(`windows/`) runs it on Windows.

```
shared/            Rust core: Event, Model, ViewModel, Effect, update/view (src/app.rs)
  src/store.rs     history (SQLite)
  src/clean.rs     threat detection, HTML sanitising, image cleaning
  src/export.rs    PDF / HTML / Markdown export
  src/ffi.rs       CoreFfi, the bytes-in/bytes-out interface exported via BoltFFI
  src/bin/codegen  generates Swift types for Event/ViewModel/Effect
apple/             SwiftUI macOS shell (Swift Package, no Xcode project)
  Sources/Klippy   App entry point, Core (effect loop), ContentView
  generated/       build output of `make generate`, not checked in
windows/           Windows shell (Rust: tao windows, wry/WebView2, tray-icon)
  src/shell.rs     event loop: runs the core and carries out its effects
  src/clipboard.rs Win32 clipboard, mapped to the core's `public.html` / `public.rtf`
  ui/              the windows' HTML, CSS and JavaScript, embedded in the exe
```

The Windows shell links the core as a Rust crate, so it needs no FFI or generated types. Its pages
send the core's `Event`s as JSON and get the `ViewModel` back as JSON. Only the events a control
can trigger are accepted from a page. `make windows-icon` regenerates `windows/assets/Klippy.ico`
from `AppIcon.svg` (on a Mac).

## Prerequisites

- Rust (stable) with the `aarch64-apple-darwin` target
- BoltFFI CLI, matching the `boltffi` crate version:
  `cargo install boltffi_cli --version '=0.30.1' --locked`
- Swift 5.9+ (Xcode Command Line Tools are enough; full Xcode is optional)
- For Windows: see [Build and run](#build-and-run)

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

## License

MIT — see [LICENSE](LICENSE).
