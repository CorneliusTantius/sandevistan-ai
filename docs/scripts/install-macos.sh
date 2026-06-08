set -eu

REPO="CorneliusTantius/sandevistan-ai"
APP_NAME="sandevistan.app"
INSTALL_DIR="/Applications"

has() { command -v "$1" >/dev/null 2>&1; }
die() { printf '%s\n' "error: $*" >&2; exit 1; }

has curl || die "curl required"
has hdiutil || die "hdiutil required"
has xattr || die "xattr required"

[ "$(uname -s)" = "Darwin" ] || die "macOS required"

case "$(uname -m)" in
  arm64) asset_suffix="mac-arm" ;;
  x86_64) asset_suffix="mac-intel" ;;
  *) die "unsupported mac architecture: $(uname -m)" ;;
esac

url="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
  | grep -Eo "https://[^\" ]+sandevistan-[^\" ]+-${asset_suffix}\.dmg" \
  | head -n1)"

[ -n "$url" ] || die "sandevistan ${asset_suffix} DMG release asset not found"

workdir="$(mktemp -d)"
mountpoint="$workdir/mount"
dmg="$workdir/sandevistan.dmg"
mkdir -p "$mountpoint"

cleanup() {
  hdiutil detach "$mountpoint" -quiet >/dev/null 2>&1 || true
  rm -rf "$workdir"
}
trap cleanup EXIT INT TERM

printf '%s\n' "downloading latest ${asset_suffix} DMG"
curl -fL "$url" -o "$dmg"

printf '%s\n' "mounting DMG"
hdiutil attach "$dmg" -nobrowse -quiet -mountpoint "$mountpoint"

app_path="$(find "$mountpoint" -maxdepth 2 -name "*.app" -type d -print -quit)"
[ -n "$app_path" ] || die "app bundle not found in DMG"

printf '%s\n' "installing ${APP_NAME} to ${INSTALL_DIR}"
if rm -rf "${INSTALL_DIR}/${APP_NAME}" 2>/dev/null && ditto "$app_path" "${INSTALL_DIR}/${APP_NAME}" 2>/dev/null; then
  :
else
  has sudo || die "sudo required to write ${INSTALL_DIR}"
  sudo rm -rf "${INSTALL_DIR}/${APP_NAME}"
  sudo ditto "$app_path" "${INSTALL_DIR}/${APP_NAME}"
fi

printf '%s\n' "removing quarantine attribute"
if xattr -dr com.apple.quarantine "${INSTALL_DIR}/${APP_NAME}" 2>/dev/null; then
  :
else
  has sudo || die "sudo required to remove quarantine"
  sudo xattr -dr com.apple.quarantine "${INSTALL_DIR}/${APP_NAME}"
fi

printf '%s\n' "installed: ${INSTALL_DIR}/${APP_NAME}"
printf '%s\n' "open it from Applications or run: open '${INSTALL_DIR}/${APP_NAME}'"
