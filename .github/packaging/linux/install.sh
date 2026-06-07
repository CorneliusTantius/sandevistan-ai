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

install_runtime_deps() {
  if command -v pacman >/dev/null 2>&1; then
    sudo pacman -S --needed --noconfirm webkit2gtk-4.1 libayatana-appindicator gtk3 librsvg openssl
  elif command -v apt-get >/dev/null 2>&1; then
    sudo apt-get update
    sudo apt-get install -y libwebkit2gtk-4.1-0 libayatana-appindicator3-1 librsvg2-2 libssl3
  elif command -v dnf >/dev/null 2>&1; then
    sudo dnf install -y webkit2gtk4.1 libayatana-appindicator-gtk3 librsvg2 openssl
  elif command -v zypper >/dev/null 2>&1; then
    sudo zypper install -y webkit2gtk-4_1 libayatana-appindicator3-1 librsvg-2-2 libopenssl3
  fi
}

install_runtime_deps || true
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
