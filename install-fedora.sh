#!/usr/bin/env bash
set -euo pipefail

sudo dnf install -y \
  gcc \
  gcc-c++ \
  make \
  curl \
  wget \
  file \
  openssl-devel \
  gtk3-devel \
  libappindicator-gtk3-devel \
  librsvg2-devel \
  webkit2gtk4.1-devel

if ! command -v rustup >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  . "$HOME/.cargo/env"
fi

if ! command -v pnpm >/dev/null 2>&1; then
  corepack enable
  corepack prepare pnpm@latest --activate
fi

pnpm install
pnpm tauri build
