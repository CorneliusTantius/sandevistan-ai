#!/usr/bin/env sh
set -eu

REPO="CorneliusTantius/sandevistan-ai"
APP_NAME="sandevistan"
DISPLAY_NAME="Sandevistan"
BIN_DIR="${HOME}/.local/bin"
DATA_DIR="${HOME}/.local/share/${APP_NAME}"
APP_DIR="${HOME}/.local/share/applications"
ICON_DIR="${HOME}/.local/share/icons/hicolor/128x128/apps"
BIN_PATH="${BIN_DIR}/${APP_NAME}"
APPIMAGE_PATH="${DATA_DIR}/${APP_NAME}.AppImage"
DESKTOP_PATH="${APP_DIR}/${APP_NAME}.desktop"
ICON_PATH="${ICON_DIR}/${APP_NAME}.png"
API="https://api.github.com/repos/${REPO}/releases/latest"
RAW_ICON="https://raw.githubusercontent.com/${REPO}/main/src-tauri/icons/128x128.png"

die() { printf '%s\n' "error: $*" >&2; exit 1; }
has() { command -v "$1" >/dev/null 2>&1; }
fetch() {
  url="$1"
  out="$2"
  if has curl; then curl -fL "$url" -o "$out"; return; fi
  if has wget; then wget -O "$out" "$url"; return; fi
  die "install curl or wget"
}
fetch_stdout() {
  url="$1"
  if has curl; then curl -fsL "$url"; return; fi
  if has wget; then wget -qO- "$url"; return; fi
  die "install curl or wget"
}

mkdir -p "$BIN_DIR" "$DATA_DIR" "$APP_DIR" "$ICON_DIR"

json="$(fetch_stdout "$API")"
asset_url="$(printf '%s' "$json" | grep -Eo '"browser_download_url": "[^"]+\.AppImage"' | head -n 1 | sed 's/"browser_download_url": "//; s/"$//')"
[ -n "$asset_url" ] || die "latest release has no AppImage asset"

tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT

printf 'downloading %s\n' "$asset_url"
fetch "$asset_url" "$tmp"
install -m 755 "$tmp" "$APPIMAGE_PATH"
cat > "$BIN_PATH" <<EOF
#!/usr/bin/env sh
export WEBKIT_DISABLE_DMABUF_RENDERER="\${WEBKIT_DISABLE_DMABUF_RENDERER:-1}"
exec "${APPIMAGE_PATH}" "\$@"
EOF
chmod 755 "$BIN_PATH"

if ! fetch "$RAW_ICON" "$ICON_PATH" >/dev/null 2>&1; then
  rm -f "$ICON_PATH"
fi

cat > "$DESKTOP_PATH" <<EOF
[Desktop Entry]
Type=Application
Name=${DISPLAY_NAME}
Comment=Minimal coding agent harness
Exec=env WEBKIT_DISABLE_DMABUF_RENDERER=1 ${BIN_PATH}
Icon=${APP_NAME}
Terminal=false
Categories=Development;Utility;
StartupWMClass=sandevistan
EOF
chmod 644 "$DESKTOP_PATH"

if has update-desktop-database; then update-desktop-database "$APP_DIR" >/dev/null 2>&1 || true; fi
if has gtk-update-icon-cache; then gtk-update-icon-cache "${HOME}/.local/share/icons/hicolor" >/dev/null 2>&1 || true; fi

printf '%s\n' "installed: ${APPIMAGE_PATH}"
printf '%s\n' "wrapper:   ${BIN_PATH}"
printf '%s\n' "launcher:  ${DESKTOP_PATH}"
printf '%s\n' "run:       ${APP_NAME}"
printf '%s\n' "desktop search should find: ${DISPLAY_NAME}"
