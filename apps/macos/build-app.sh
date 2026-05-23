#!/usr/bin/env bash
#
# build-app.sh — ClipNoteX.app バンドルを生成する。
#
# 使い方:
#   ./build-app.sh [--debug]
#
# 出力:
#   apps/macos/build/ClipNoteX.app
#
# 手順:
#   1) cargo build (release|debug) -p clipnotex-ffi
#   2) cbindgen が生成したヘッダを Sources/ClipNoteXCore/ に同期
#   3) swift build (release|debug)
#   4) .app ディレクトリツリーを作成して実行ファイルと Info.plist を配置
set -euo pipefail

CONFIG="release"
if [[ "${1:-}" == "--debug" ]]; then CONFIG="debug"; fi

cd "$(dirname "$0")"
REPO_ROOT="$(cd ../.. && pwd)"
APP_DIR="build/ClipNoteX.app"
CONTENTS="${APP_DIR}/Contents"
MACOS_BIN="${CONTENTS}/MacOS"
RES="${CONTENTS}/Resources"

echo "→ cargo build (${CONFIG}) -p clipnotex-ffi"
(
  cd "${REPO_ROOT}"
  if [[ "${CONFIG}" == "release" ]]; then
    cargo build --release -p clipnotex-ffi
  else
    cargo build -p clipnotex-ffi
  fi
)

# Sync cbindgen-generated header into the Swift module location.
HEADER_SRC="${REPO_ROOT}/crates/clipnotex-ffi/include/ClipNoteX.h"
HEADER_DST="Sources/ClipNoteXCore/ClipNoteX.h"
if [[ -f "${HEADER_SRC}" ]]; then
  cp "${HEADER_SRC}" "${HEADER_DST}"
  echo "→ synced ClipNoteX.h"
fi

echo "→ swift build (${CONFIG})"
if [[ "${CONFIG}" == "release" ]]; then
  swift build -c release
  SWIFT_BIN=".build/release/ClipNoteX"
else
  swift build
  SWIFT_BIN=".build/debug/ClipNoteX"
fi

echo "→ assembling ${APP_DIR}"
# 重要: アプリディレクトリを完全削除してからアセンブル。中身を残すと
# 古い実行ファイルの quarantine 属性や署名が残って権限を再要求される。
rm -rf "${APP_DIR}"
mkdir -p "${MACOS_BIN}" "${RES}"
cp "${SWIFT_BIN}" "${MACOS_BIN}/ClipNoteX"
cp Info.plist "${CONTENTS}/Info.plist"
chmod +x "${MACOS_BIN}/ClipNoteX"

# Optional icon (PlaceholderApp.icns if present)
if [[ -f icon.icns ]]; then
  cp icon.icns "${RES}/icon.icns"
fi

# Quarantine 属性を剥がす (cp が継承するため、署名前に必須)。
# これが残っていると macOS が「ダウンロードされたアプリ」扱いで
# 起動のたび確認ダイアログ + 権限リセットになる。
xattr -cr "${APP_DIR}" 2>/dev/null || true

# ── Ad-hoc code signing ──────────────────────────────────────────────────
# Even without a Developer ID cert, an ad-hoc signature lets the binary
# pass the LaunchServices verification on the build machine. Without ANY
# signature, recent macOS versions refuse to launch downloaded .app
# bundles entirely.
#
# To use a real Developer ID instead, run `./sign-app.sh "..."` after
# build-app.sh, OR set CNX_SIGN_IDENTITY env to a cert name and we'll use
# it here.
SIGN_ID="${CNX_SIGN_IDENTITY:--}" # "-" = ad-hoc
ENT_FLAGS=()
if [[ -f ClipNoteX.entitlements ]]; then
  ENT_FLAGS=(--entitlements ClipNoteX.entitlements)
fi
codesign --force --deep \
  --sign "${SIGN_ID}" \
  --options runtime \
  --timestamp=none \
  "${ENT_FLAGS[@]}" \
  "${APP_DIR}" 2>/dev/null && {
    if [[ "${SIGN_ID}" == "-" ]]; then
      echo "→ ad-hoc signed (no Developer ID)"
    else
      echo "→ signed with ${SIGN_ID}"
    fi
  } || echo "⚠ codesign step skipped/failed (non-fatal)"

echo ""
echo "✅ Built ${APP_DIR}"
echo "   Run: open ${APP_DIR}"
echo "   Or:  ${MACOS_BIN}/ClipNoteX (foreground, logs to stderr)"
echo ""
echo "ℹ For distribution, run ./sign-app.sh \"Developer ID Application: ...\" --notarize"
