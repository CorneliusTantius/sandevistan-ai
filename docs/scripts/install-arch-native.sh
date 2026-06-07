set -eu

REPO="CorneliusTantius/sandevistan-ai"

has() { command -v "$1" >/dev/null 2>&1; }
die() { printf '%s\n' "error: $*" >&2; exit 1; }

has curl || die "curl required"
has tar || die "tar required"
has sudo || die "sudo required"

url="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
  | grep -Eo 'https://[^" ]+sandevistan-[^" ]+\.tar\.gz' \
  | head -n1)"

[ -n "$url" ] || die "sandevistan tar.gz release asset not found"

workdir="$(mktemp -d)"
trap 'rm -rf "$workdir"' EXIT

printf '%s\n' "downloading latest release"
curl -fsSL "$url" -o "$workdir/sandevistan.tar.gz"

tar -xzf "$workdir/sandevistan.tar.gz" -C "$workdir"
cd "$workdir/sandevistan"

printf '%s\n' "installing sandevistan"
./install.sh

printf '%s\n' "installed native package: sandevistan"
printf '%s\n' "run: sandevistan"
