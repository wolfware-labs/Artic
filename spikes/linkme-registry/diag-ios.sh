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

echo "== link map and load reasons for the release force_load link"
target=aarch64-apple-ios-sim
cc=(xcrun -sdk iphonesimulator clang -target arm64-apple-ios15.0-simulator harness/static_main.c)
"${cc[@]}" "$lib" -Wl,-force_load,"$lib" -Wl,-why_load -Wl,-map,out/map.txt -o out/diag-map 2>&1 | grep -iE "entries|registry" | head -10
echo "-- map lines for the linkme section and entry symbols"
grep -E "__linkme|BUTTON|GRID|STACKPANEL|TEXTBLOCK|ROOT" out/map.txt | head -20

echo "== other load strategies (release profile)"
udid="$(xcrun simctl list devices booted -j | python3 -c 'import json,sys; d=json.load(sys.stdin)["devices"]; print([x["udid"] for v in d.values() for x in v][0])')"
try() {
  local label="$1"; shift
  "${cc[@]}" "$@" -o out/diag-try 2>/dev/null || { echo "$label: link failed"; return; }
  echo "$label: $(xcrun simctl spawn "$udid" "$PWD/out/diag-try" | cut -c1-40)"
}
try "all_load" "$lib" -Wl,-all_load
try "u BUTTON symbol" "$lib" -Wl,-u,__RNvCseSu9cqjfuuk_15spike_entries_a6BUTTON
try "force_load, no positional lib" -Wl,-force_load,"$lib"
try "ld_classic + force_load" -Wl,-ld_classic -Wl,-force_load,"$lib"
