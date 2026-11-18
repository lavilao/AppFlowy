#!/usr/bin/env bash
#
# Post-build sanity check that would have caught the Redmi 9A crash:
#
#   Unhandled Exception: Invalid argument(s): Failed to load dynamic library
#   'libdart_ffi.so': dlopen failed: cannot locate symbol
#   "_ZTTNSt6__ndk118basic_stringstreamIcNS_11char_traitsIcEENS_9allocatorIcEEEE"
#
# libdart_ffi.so (the Rust backend, linked by cargo-ndk) resolves its libc++
# (__ndk1 namespace) symbols against the libc++_shared.so packaged in the SAME
# APK at dlopen() time. If the packaged runtime is older than the one the Rust
# lib was linked against (e.g. an NDK r24 copy next to an r27d-linked
# libdart_ffi.so), every launch dies before the first Flutter frame.
#
# Usage: check_apk_libcxx_symbols.sh <apk> [abi]        (default ABI: armeabi-v7a)
#        READELF=/path/to/llvm-readelf (falls back to `readelf`)
#
# Checks:
#   1. The packaged libc++_shared.so exports modern libc++ symbols
#      ("stringstream" count must be > 0 — the r24-era runtime exports none).
#   2. Every undefined __ndk1 symbol of libdart_ffi.so is exported by the
#      packaged libc++_shared.so (scope is __ndk1 only: other UND symbols
#      resolve against libc/liblog/libz/... which are NOT part of the C++
#      runtime and would false-positive).
#
# Exit code 0 = the APK's native runtime is consistent; non-zero = broken APK.

set -euo pipefail

APK="${1:?usage: check_apk_libcxx_symbols.sh <apk> [abi]}"
ABI="${2:-armeabi-v7a}"
READELF="${READELF:-readelf}"

if [ ! -f "$APK" ]; then
  echo "::error::APK not found: $APK"
  exit 1
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

if ! unzip -o -q "$APK" "lib/$ABI/libdart_ffi.so" "lib/$ABI/libc++_shared.so" -d "$WORK" 2>/dev/null; then
  echo "::error::APK does not contain lib/$ABI/libdart_ffi.so and/or lib/$ABI/libc++_shared.so"
  exit 1
fi

FFI="$WORK/lib/$ABI/libdart_ffi.so"
CXX="$WORK/lib/$ABI/libc++_shared.so"

# ---------------------------------------------------------------------------
# Check 1: the packaged libc++_shared.so must be the modern (r27d-era) runtime
# ---------------------------------------------------------------------------
STREAM_COUNT="$("$READELF" --dyn-syms -W "$CXX" | grep -c stringstream || true)"
echo "libc++_shared.so ($ABI): exported stringstream symbols: $STREAM_COUNT"
if [ "$STREAM_COUNT" -eq 0 ]; then
  echo "::error::packaged libc++_shared.so ($ABI) is an OLD libc++ runtime (0 stringstream exports, NDK r24 era). libdart_ffi.so needs the NDK r27d runtime — check ndkVersion in android/app/build.gradle and the NDK pin in the CI workflow."
  exit 1
fi

# ---------------------------------------------------------------------------
# Check 2: all __ndk1 UND symbols of libdart_ffi.so must be exported
# ---------------------------------------------------------------------------
UND_NDK1_COUNT="$("$READELF" --dyn-syms -W "$FFI" | awk '$7=="UND" && $8 ~ /__ndk1/ {n++} END {print n+0}')"
echo "libdart_ffi.so  ($ABI): undefined __ndk1 symbols: $UND_NDK1_COUNT"

MISSING="$(comm -23 \
  <("$READELF" --dyn-syms -W "$FFI" | awk '$7=="UND" && $8 ~ /__ndk1/ {print $8}' | sort -u) \
  <("$READELF" --dyn-syms -W "$CXX" | awk '$7!="UND" {print $8}' | sed 's/@.*//' | sort -u))"

if [ -n "$MISSING" ]; then
  MISSING_COUNT="$(printf '%s\n' "$MISSING" | wc -l)"
  echo "::error::libdart_ffi.so ($ABI) references $MISSING_COUNT libc++ symbol(s) that the packaged libc++_shared.so does not export — dlopen WILL fail on device:"
  printf '%s\n' "$MISSING"
  echo ""
  echo "Fix: package the libc++_shared.so from the SAME NDK that built libdart_ffi.so (ndkVersion in android/app/build.gradle) and make sure it wins the merge."
  exit 1
fi

echo "OK: packaged libc++_shared.so satisfies all __ndk1 symbol dependencies of libdart_ffi.so ($ABI)."
