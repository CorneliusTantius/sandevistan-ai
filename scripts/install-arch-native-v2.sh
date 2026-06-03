#!/usr/bin/env sh
set -eu

REPO="CorneliusTantius/sandevistan-ai"
BRANCH="${BRANCH:-main}"

has() { command -v "$1" >/dev/null 2>&1; }
die() { printf '%s\n' "error: $*" >&2; exit 1; }

has pacman || die "Arch/pacman required"
has makepkg || die "install base-devel first"

printf '%s\n' "installing build/runtime deps"
sudo pacman -S --needed --noconfirm \
  base-devel git nodejs npm \
  webkit2gtk-4.1 libayatana-appindicator gtk3 librsvg

if ! has cargo || ! has rustc; then
  if has rustup; then
    printf '%s\n' "rustup found; installing/selecting stable toolchain"
    rustup default stable
  else
    printf '%s\n' "installing rust toolchain"
    sudo pacman -S --needed --noconfirm rust
  fi
fi

has cargo || die "cargo not found after rust setup"
has rustc || die "rustc not found after rust setup"

workdir="$(mktemp -d)"
trap 'rm -rf "$workdir"' EXIT
cd "$workdir"

printf '%s\n' "writing PKGBUILD for branch ${BRANCH}"
cat > PKGBUILD <<EOF
# Maintainer: CorneliusTantius
pkgname=sandevistan-git
pkgver=1.2.0.r0.g0000000
pkgrel=1
pkgdesc="Minimal coding agent harness"
arch=(x86_64)
url="https://github.com/${REPO}"
license=(MIT)
depends=(webkit2gtk-4.1 libayatana-appindicator gtk3 librsvg)
makedepends=(git nodejs npm)
provides=(sandevistan)
conflicts=(sandevistan)
source=("git+https://github.com/${REPO}.git#branch=${BRANCH}")
sha256sums=(SKIP)

pkgver() {
  cd "\${srcdir}/sandevistan-ai"
  git describe --tags --long --always | sed 's/^v//;s/-/.r/;s/-/./'
}

build() {
  cd "\${srcdir}/sandevistan-ai"
  command -v cargo >/dev/null || { echo "cargo missing; install rust or rustup default stable" >&2; return 1; }
  command -v rustc >/dev/null || { echo "rustc missing; install rust or rustup default stable" >&2; return 1; }
  npm ci
  npm run build
  cargo build --manifest-path src-tauri/Cargo.toml --release
}

package() {
  cd "\${srcdir}/sandevistan-ai"

  install -Dm755 "src-tauri/target/release/sandevistan" "\${pkgdir}/usr/lib/sandevistan/sandevistan-bin"
  install -Dm644 "src-tauri/icons/128x128.png" "\${pkgdir}/usr/share/icons/hicolor/128x128/apps/sandevistan.png"

  install -Dm755 /dev/stdin "\${pkgdir}/usr/bin/sandevistan" <<'WRAP'
#!/usr/bin/env sh
export WEBKIT_DISABLE_DMABUF_RENDERER="\${WEBKIT_DISABLE_DMABUF_RENDERER:-1}"
export WEBKIT_DISABLE_COMPOSITING_MODE="\${WEBKIT_DISABLE_COMPOSITING_MODE:-1}"
exec /usr/lib/sandevistan/sandevistan-bin "\$@"
WRAP

  install -Dm644 /dev/stdin "\${pkgdir}/usr/share/applications/sandevistan.desktop" <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=Sandevistan
Comment=Minimal coding agent harness
Exec=sandevistan
Icon=sandevistan
Terminal=false
Categories=Development;Utility;
StartupWMClass=sandevistan
DESKTOP
}
EOF

makepkg -i --noconfirm

printf '%s\n' "installed native package: sandevistan"
printf '%s\n' "run: sandevistan"
