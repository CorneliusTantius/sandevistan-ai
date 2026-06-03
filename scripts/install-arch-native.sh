#!/usr/bin/env sh
set -eu

REPO="CorneliusTantius/sandevistan-ai"
BRANCH="${BRANCH:-main}"
PKGBUILD_URL="https://raw.githubusercontent.com/${REPO}/${BRANCH}/packaging/arch/PKGBUILD"

has() { command -v "$1" >/dev/null 2>&1; }
die() { printf '%s\n' "error: $*" >&2; exit 1; }
fetch() {
  url="$1"
  out="$2"
  if has curl; then curl -fL -H 'Cache-Control: no-cache' "$url" -o "$out"; return; fi
  if has wget; then wget --no-cache -O "$out" "$url"; return; fi
  die "install curl or wget"
}

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

printf '%s\n' "downloading PKGBUILD from ${PKGBUILD_URL}"
fetch "$PKGBUILD_URL" PKGBUILD

if [ "$BRANCH" != "main" ]; then
  sed -i "s/#branch=main/#branch=${BRANCH}/" PKGBUILD
fi

makepkg -i --noconfirm

printf '%s\n' "installed native package: sandevistan"
printf '%s\n' "run: sandevistan"
