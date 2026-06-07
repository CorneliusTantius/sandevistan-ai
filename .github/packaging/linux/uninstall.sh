#!/usr/bin/env sh
set -eu

APP_NAME="sandevistan"
PREFIX="${PREFIX:-${HOME}/.local}"

rm -f "${PREFIX}/bin/${APP_NAME}"
rm -rf "${PREFIX}/share/${APP_NAME}"
rm -f "${PREFIX}/share/applications/${APP_NAME}.desktop"
rm -f "${PREFIX}/share/icons/hicolor/128x128/apps/${APP_NAME}.png"

if command -v update-desktop-database >/dev/null 2>&1; then update-desktop-database "${PREFIX}/share/applications" >/dev/null 2>&1 || true; fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then gtk-update-icon-cache "${PREFIX}/share/icons/hicolor" >/dev/null 2>&1 || true; fi

printf '%s\n' "uninstalled: ${APP_NAME}"
