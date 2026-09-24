#!/usr/bin/env bash
# Builds libspike_ffi.a for the iOS simulator under every profile, links it into
# a C harness with and without dead-code stripping, and runs it in a booted
# simulator. Requires macOS with Xcode. Prints one markdown table row each.
set -euo pipefail
cd "$(dirname "$0")"
target=aarch64-apple-ios-sim
rustup target add "$target" >/dev/null

udid="$(xcrun simctl list devices available -j | python3 -c '
import json, sys
devs = json.load(sys.stdin)["devices"]
for runtime, items in devs.items():
    if "iOS" in runtime:
        for d in items:
            if d["name"].startswith("iPhone"):
                print(d["udid"]); raise SystemExit
')"
xcrun simctl boot "$udid" 2>/dev/null || true
xcrun simctl bootstatus "$udid" -b >/dev/null

mkdir -p out
for profile in dev release fat thin cgu1; do
  cargo build -q -p spike-ffi --target "$target" --profile "$profile"
  dir="$profile"; [ "$profile" = dev ] && dir=debug
  lib="target/$target/$dir/libspike_ffi.a"
  for strip in none dead_strip; do
    flags=(); [ "$strip" = dead_strip ] && flags=(-Wl,-dead_strip)
    exe="out/ios-$profile-$strip"
    xcrun -sdk iphonesimulator clang -target arm64-apple-ios15.0-simulator \
      harness/static_main.c "$lib" "${flags[@]}" -o "$exe"
    out="$(xcrun simctl spawn "$udid" "$PWD/$exe" || true)"
    n="$(sed -nE 's/^entries=([0-9]+).*/\1/p' <<<"$out")"; n="${n:-0}"
    result="pass"; [ "$n" = "5" ] || result="FAIL"
    echo "| iOS simulator (staticlib) | $profile | $strip | $n/5 | $result |"
  done
done
