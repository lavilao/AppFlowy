#!/usr/bin/env bash
#
# Fetches the Cactus runtime that the on-device intelligence features use.
#
#   ./scripts/fetch_cactus.sh engine    # libcactus.a + cactus.h per Android ABI
#   ./scripts/fetch_cactus.sh weights   # needle3.cact, whistle.cact, mirage.cact
#   ./scripts/fetch_cactus.sh all       # both (what CI runs)
#
# Nothing here is committed: the static engine is 5 MB and the three model
# archives are 74 MB, so the build downloads them and verifies each file
# against a pinned SHA-256. Everything comes from the Apache-2.0 Cactus
# release on Hugging Face (see assets/cactus/README.md).
set -euo pipefail

ENGINE_VERSION="4.1.4"
ENGINE_REVISION="v${ENGINE_VERSION}"
HUGGING_FACE="https://huggingface.co"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FRONTEND_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
ENGINE_DIR="${FRONTEND_DIR}/appflowy_flutter/android/app/src/main/cactus"
WEIGHTS_DIR="${FRONTEND_DIR}/appflowy_flutter/assets/cactus"

log() { echo "🛠️  $*"; }

download() {
  # download <url> <destination> <sha256> <label>
  local url="$1" destination="$2" expected="$3" label="$4"
  if [ -f "${destination}" ] && [ "$(sha256sum "${destination}" | cut -d' ' -f1)" = "${expected}" ]; then
    log "${label}: already present and verified"
    return 0
  fi
  log "${label}: downloading"
  mkdir -p "$(dirname "${destination}")"
  curl -sSL --retry 3 --retry-delay 2 -o "${destination}.part" "${url}"
  local actual
  actual="$(sha256sum "${destination}.part" | cut -d' ' -f1)"
  if [ "${actual}" != "${expected}" ]; then
    echo "ERROR: ${label} checksum mismatch" >&2
    echo "  expected ${expected}" >&2
    echo "  actual   ${actual}" >&2
    rm -f "${destination}.part"
    exit 1
  fi
  mv "${destination}.part" "${destination}"
  log "${label}: verified"
}

fetch_engine() {
  # arch (Hugging Face folder) -> ABI used by the Android build
  local arch abi
  for pair in "android-armv7:armeabi-v7a" "android-arm64:arm64-v8a"; do
    arch="${pair%%:*}"
    abi="${pair##*:}"
    case "${arch}" in
      android-armv7)
        lib_sha="6f7daade8587c714a2f46fdccf90535660b78d22e3c4a89518d53ea857568afa" ;;
      android-arm64)
        lib_sha="67dbd7b856a3e9d8b0fbb68f0c59592e7572cf859238ba7f3074f74b6fa9a5da" ;;
    esac
    header_sha="9be7eb25b1d89180495d04a0d2c27053f282dac85add50cacc48d73302819d9d"
    download \
      "${HUGGING_FACE}/Cactus-Compute/cactus/resolve/${ENGINE_REVISION}/releases/${ENGINE_VERSION}/${arch}/libcactus.a" \
      "${ENGINE_DIR}/${abi}/libcactus.a" "${lib_sha}" "engine ${abi}"
    download \
      "${HUGGING_FACE}/Cactus-Compute/cactus/resolve/${ENGINE_REVISION}/releases/${ENGINE_VERSION}/${arch}/cactus.h" \
      "${ENGINE_DIR}/${abi}/cactus.h" "${header_sha}" "cactus.h ${abi}"
  done
}

fetch_weights() {
  download "${HUGGING_FACE}/Cactus-Compute/needle3/resolve/main/needle3.cact" \
    "${WEIGHTS_DIR}/needle3.cact" \
    "c9d915eca282ed42d1a09b143b592adb4cc6744ffe2d294adf5cfc5548170c38" "needle3.cact"
  download "${HUGGING_FACE}/Cactus-Compute/whistle/resolve/main/whistle.cact" \
    "${WEIGHTS_DIR}/whistle.cact" \
    "b6e02f048568ac5d01a2042556c658061e699acbc0aa2a1439f52f3d461dffeb" "whistle.cact"
  download "${HUGGING_FACE}/Cactus-Compute/mirage/resolve/main/mirage.cact" \
    "${WEIGHTS_DIR}/mirage.cact" \
    "4249a0f7515272ff76b9414bec17d7e7daf84b15b66c7fb4f17fce1e66f9f237" "mirage.cact"

  # The manifest is what the Dart side reads to decide which on-device
  # features to offer: it is only written when the weights really are present,
  # so a build without them simply hides the actions instead of failing.
  cat > "${WEIGHTS_DIR}/manifest.json" <<EOF
{
  "engine": "${ENGINE_VERSION}",
  "models": {
    "needle3": {
      "asset": "assets/cactus/needle3.cact",
      "bytes": 35335380,
      "sha256": "c9d915eca282ed42d1a09b143b592adb4cc6744ffe2d294adf5cfc5548170c38"
    },
    "whistle": {
      "asset": "assets/cactus/whistle.cact",
      "bytes": 16919407,
      "sha256": "b6e02f048568ac5d01a2042556c658061e699acbc0aa2a1439f52f3d461dffeb"
    },
    "mirage": {
      "asset": "assets/cactus/mirage.cact",
      "bytes": 21611043,
      "sha256": "4249a0f7515272ff76b9414bec17d7e7daf84b15b66c7fb4f17fce1e66f9f237"
    }
  }
}
EOF
  log "manifest.json written"
  ls -l "${WEIGHTS_DIR}"
}

case "${1:-all}" in
  engine) fetch_engine ;;
  weights) fetch_weights ;;
  all) fetch_engine; fetch_weights ;;
  *) echo "usage: $0 [engine|weights|all]" >&2; exit 2 ;;
esac

log "Cactus is ready"
