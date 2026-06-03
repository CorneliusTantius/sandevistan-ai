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
  if has curl; then curl -fL "$url" -o "$out"; return; fi
  if has wget; then wget -O "$out" "$url"; return; fi
  die "install curl or wget"
}

has pacman || die "Arch/pacman required"
has makepkg || die "install base-devel first"

printf '%s\n' "installing build/runtime deps"
sudo pacman -S --needed --noconfirm \
  base-devel git nodejs npm rust \
  webkit2gtk-4.1 libayatana-appindicator gtk3 librsvg

workdir="$(mktemp -d)"
trap 'rm -rf "$workdir"' EXIT
cd "$workdir"

printf '%s\n' "downloading PKGBUILD from ${PKGBUILD_URL}"
fetch "$PKGBUILD_URL" PKGBUILD

if [ "$BRANCH" != "main" ]; then
  sed -i "s/#branch=main/#branch=${BRANCH}/" PKGBUILD
fi

makepkg -si --noconfirm

printf '%s\n' "installed native package: sandevistan"
printf '%s\n' "run: sandevistan"
