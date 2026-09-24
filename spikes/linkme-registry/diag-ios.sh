#!/usr/bin/env bash
# Inspects the release iOS static library and a linked harness to show where
# the entries from spike-entries-a and spike-entries-b go. Run after run-ios.sh.
set -uo pipefail
cd "$(dirname "$0")"
lib=target/aarch64-apple-ios-sim/release/libspike_ffi.a

echo "== archive members that mention the entry crates"
ar t "$lib" | grep -iE "entries|registry" || echo "(none)"

echo "== symbols for the entry statics in the archive"
nm -m "$lib" 2>/dev/null | grep -E "BUTTON|TEXTBLOCK|GRID|STACKPANEL|ROOT" || echo "(none)"

echo "== linkme sections in the archive objects"
otool -l "$lib" 2>/dev/null | grep -E "^.*\.o\)?:$|sectname __linkme|segname" | grep -B1 "__linkme" | head -40 || true

echo "== linkme symbols in the archive"
nm -m "$lib" 2>/dev/null | grep -i linkme | head -40 || echo "(none)"

echo "== linkme sections in the linked harness (force_load)"
otool -l out/ios-release-force_load 2>/dev/null | grep -A5 "sectname __linkme" | head -40 || echo "(none)"
