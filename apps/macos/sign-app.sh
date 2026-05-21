#!/usr/bin/env bash
#
# sign-app.sh — Code signing + (optional) notarization for ClipNoteX.app.
#
# Usage:
#   ./sign-app.sh "Developer ID Application: Your Name (TEAMID)" [--notarize]
#
# Prerequisites:
#   - macOS with Xcode CLT
#   - Valid "Developer ID Application" certificate in your login Keychain
#   - For --notarize:
#       xcrun notarytool store-credentials "AC_PASSWORD" \
#         --apple-id you@example.com --team-id TEAMID --password "<app-specific>"
#
# Steps:
#   1) codesign --deep --options runtime --entitlements ClipNoteX.entitlements
#   2) (optional) zip + xcrun notarytool submit --wait
#   3) (optional) xcrun stapler staple
set -euo pipefail

if [[ $# -lt 1 ]]; then
    echo "Usage: $0 \"Developer ID Application: ...\" [--notarize]"
    exit 1
fi

IDENTITY="$1"
NOTARIZE=0
if [[ "${2:-}" == "--notarize" ]]; then NOTARIZE=1; fi

cd "$(dirname "$0")"
APP="build/ClipNoteX.app"

if [[ ! -d "${APP}" ]]; then
    echo "❌ ${APP} not found. Run ./build-app.sh first."
    exit 1
fi

echo "→ codesign (hardened runtime) with: ${IDENTITY}"
codesign --force --deep \
    --sign "${IDENTITY}" \
    --options runtime \
    --timestamp \
    --entitlements ClipNoteX.entitlements \
    "${APP}"

echo "→ codesign --verify"
codesign --verify --deep --strict --verbose=2 "${APP}"

echo "→ spctl --assess (Gatekeeper check)"
spctl --assess --type execute --verbose "${APP}" || true

if [[ "${NOTARIZE}" == "1" ]]; then
    ZIP="build/ClipNoteX.zip"
    echo "→ zip → ${ZIP}"
    rm -f "${ZIP}"
    /usr/bin/ditto -c -k --keepParent "${APP}" "${ZIP}"

    echo "→ xcrun notarytool submit (keychain profile: AC_PASSWORD)"
    xcrun notarytool submit "${ZIP}" \
        --keychain-profile "AC_PASSWORD" \
        --wait

    echo "→ xcrun stapler staple"
    xcrun stapler staple "${APP}"
    echo "✅ Notarized & stapled."
else
    echo ""
    echo "✅ Signed (not notarized). Pass --notarize to also submit to Apple."
fi
