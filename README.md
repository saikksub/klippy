# Klippy

A [Crux](https://github.com/redbadger/crux) app: the behaviour lives in a Rust core
(`shared/`), and a SwiftUI shell (`apple/`) runs it as a native macOS desktop app.

```
shared/            Rust core: Event, Model, ViewModel, Effect, update/view (src/app.rs)
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
| `make run` | generate, build, bundle `apple/build/Klippy.app` and open it |
| `make test` | Rust core tests + Swift shell tests against the real core |
| `make app CONFIG=release` | optimized `.app` bundle |
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
