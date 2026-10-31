#!/bin/sh
# Builds a release Klippy-<version>.dmg in apple/build.noindex/ that you can
# keep, or open on any Mac and drag Klippy into Applications.
set -eu

cd "$(dirname "$0")/.."
./scripts/check-tools.sh

make dmg

DMG=$(ls -t apple/build.noindex/Klippy-*.dmg | head -1)
echo
echo "Done: $DMG"
echo "Open it and drag Klippy onto Applications."
