#!/bin/sh
# Checks for everything the build needs and says how to install what's
# missing. Used by the other scripts.
set -eu

missing=0
need() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "missing: $1 ($2)"
        missing=1
    fi
}

need cargo "install Rust from https://rustup.rs"
need swift "run: xcode-select --install"
need boltffi "run: cargo install boltffi_cli --version '=0.30.1' --locked"

if command -v rustup >/dev/null 2>&1 \
    && ! rustup target list --installed | grep -q aarch64-apple-darwin; then
    echo "missing: Rust target aarch64-apple-darwin (run: rustup target add aarch64-apple-darwin)"
    missing=1
fi

if ! security find-identity -v -p codesigning 2>/dev/null | grep -q '"Klippy Development"'; then
    echo "note: no \"Klippy Development\" certificate; the app will be signed ad hoc."
    echo "      Run ./scripts/make-dev-cert.sh once to create it."
fi

[ "$missing" -eq 0 ] || exit 1
