#!/usr/bin/env bash
# Runs the x86_64 libraries from run-android-build.sh on a running emulator
# through the dlopen harness. Prints one markdown table row each.
set -uo pipefail
cd "$(dirname "$0")"
adb push out/dlopen-x86_64 /data/local/tmp/ >/dev/null
for profile in dev release fat thin cgu1; do
  adb push "out/x86_64-linux-android-$profile.so" /data/local/tmp/libspike_ffi.so >/dev/null
  out="$(adb shell 'cd /data/local/tmp && chmod +x dlopen-x86_64 && ./dlopen-x86_64 ./libspike_ffi.so')"
  n="$(sed -nE 's/^entries=([0-9]+).*/\1/p' <<<"$out" | tr -d '\r')"; n="${n:-0}"
  result="pass"; [ "$n" = "5" ] || result="FAIL"
  echo "| Android x86_64 emulator (cdylib, dlopen) | $profile | runtime | $n/5 | $result |"
done
