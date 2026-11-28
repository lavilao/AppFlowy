#!/usr/bin/env bash
#
# Patches the pinned appflowy_editor revision so it compiles against Flutter
# 3.50.
#
# Why this exists
# ---------------
# Flutter 3.50 added `TextInputClient.onFocusReceived`. Any *concrete* class
# that implements TextInputClient must now provide it, and the editor's
# `DeltaTextInputService` (lib/src/editor/editor_component/service/ime/
# delta_input_service.dart) does not:
#
#   Error: The non-abstract class 'DeltaTextInputService' is missing
#   implementations for these members:
#    - TextInputClient.onFocusReceived
#
# The fix is in the editor upstream (commit 41335c54a, "chore: merge flutter
# 3_27_4 into main branch", 2026-07-27), but the app pins the editor to
# 470c4e77c (2025-05-07): jumping fourteen months of editor API changes is a
# migration of its own, so this adds the one missing method to the checkout
# instead. It is idempotent, and it is a bridge — the proper fix is dropping
# this script when the app moves to a newer editor revision.
#
# Usage: ./scripts/patch_appflowy_editor.sh   (run after `flutter pub get`;
# the CI workflow runs it before `flutter build apk`)
set -euo pipefail

GIT_REF="470c4e77c71b63f693ce0923a927afcd667d6f3b"
TARGET="lib/src/editor/editor_component/service/ime/delta_input_service.dart"
METHOD="  @override
  bool onFocusReceived() => false;
"

log() { echo "🩹 $*"; }

# The git dependency is unpacked as <pub-cache>/git/appflowy-editor-<sha>/.
# PUB_CACHE may be unset; `flutter pub get` defaults to ~/.pub-cache.
PUB_CACHE_DIR="${PUB_CACHE:-${HOME}/.pub-cache}"
candidates=()
while IFS= read -r dir; do
  candidates+=("${dir}")
done < <(find "${PUB_CACHE_DIR}/git" -maxdepth 1 -type d -name 'appflowy-editor*' 2>/dev/null | sort)

if [ "${#candidates[@]}" -eq 0 ]; then
  echo "ERROR: no appflowy_editor checkout under ${PUB_CACHE_DIR}/git — run 'flutter pub get' first" >&2
  exit 1
fi

patched=0
for root in "${candidates[@]}"; do
  file="${root}/${TARGET}"
  if [ ! -f "${file}" ]; then
    continue
  fi
  if grep -q "onFocusReceived" "${file}"; then
    log "$(basename "${root}"): already patched"
    patched=$((patched + 1))
    continue
  fi

  # Insert the override as the first member of the class body. The class
  # declaration is stable in the pinned revision, so this is a simple
  # one-time splice rather than a parser.
  python3 - "${file}" <<'PY'
import sys

path = sys.argv[1]
method = "  @override\n  bool onFocusReceived() => false;\n"
with open(path) as handle:
    source = handle.read()

anchor = "class DeltaTextInputService extends TextInputService with DeltaTextInputClient {\n"
if anchor not in source:
    raise SystemExit(f"anchor not found in {path}")
source = source.replace(anchor, anchor + method, 1)
with open(path, "w") as handle:
    handle.write(source)
PY
  log "$(basename "${root}"): added TextInputClient.onFocusReceived to DeltaTextInputService"
  patched=$((patched + 1))
done

if [ "${patched}" -eq 0 ]; then
  echo "ERROR: ${TARGET} was not found in any appflowy-editor checkout" >&2
  exit 1
fi

log "appflowy_editor is ready (pinned revision ${GIT_REF})"
