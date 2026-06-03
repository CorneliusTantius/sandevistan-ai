#!/usr/bin/env sh
set -eu

APP_NAME="sandevistan"
DISPLAY_NAME="Sandevistan"
PREFIX="${PREFIX:-${HOME}/.local}"
BASE_DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"

BIN_DIR="${PREFIX}/bin"
DATA_DIR="${PREFIX}/share/${APP_NAME}"
APP_DIR="${PREFIX}/share/applications"
ICON_DIR="${PREFIX}/share/icons/hicolor/128x128/apps"

mkdir -p "$BIN_DIR" "$DATA_DIR" "$APP_DIR" "$ICON_DIR"

install -m 755 "${BASE_DIR}/bin/sandevistan-bin" "${DATA_DIR}/sandevistan-bin"
sed "s|@PREFIX@|${PREFIX}|g" "${BASE_DIR}/bin/sandevistan" > "${BIN_DIR}/sandevistan"
chmod 755 "${BIN_DIR}/sandevistan"
install -m 644 "${BASE_DIR}/share/icons/hicolor/128x128/apps/sandevistan.png" "${ICON_DIR}/sandevistan.png"

sed "s|@PREFIX@|${PREFIX}|g" "${BASE_DIR}/share/applications/sandevistan.desktop" > "${APP_DIR}/sandevistan.desktop"
chmod 644 "${APP_DIR}/sandevistan.desktop"

if command -v update-desktop-database >/dev/null 2>&1; then update-desktop-database "$APP_DIR" >/dev/null 2>&1 || true; fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then gtk-update-icon-cache "${PREFIX}/share/icons/hicolor" >/dev/null 2>&1 || true; fi

printf '%s\n' "installed: ${BIN_DIR}/sandevistan"
printf '%s\n' "launcher:  ${APP_DIR}/sandevistan.desktop"
printf '%s\n' "run:       sandevistan"
