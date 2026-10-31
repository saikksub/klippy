#!/bin/sh
# Builds a release Klippy and installs it to /Applications, replacing (and
# relaunching) any copy already there. Run it again to update.
#
# Each new build has a new code hash, so macOS asks twice for Keychain access
# the first time it runs; enter your password and choose Always Allow both
# times.
set -eu

cd "$(dirname "$0")/.."
./scripts/check-tools.sh

make app CONFIG=release

osascript -e 'quit app "Klippy"' 2>/dev/null || true
# Give it a moment to quit before its bundle is replaced.
while pgrep -xq Klippy; do sleep 0.2; done

rm -rf /Applications/Klippy.app
cp -R apple/build.noindex/Klippy.app /Applications/
open /Applications/Klippy.app

echo
echo "Installed /Applications/Klippy.app"
