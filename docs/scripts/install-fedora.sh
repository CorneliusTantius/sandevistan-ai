set -eu

REPO="CorneliusTantius/sandevistan-ai"

has() { command -v "$1" >/dev/null 2>&1; }
die() { printf '%s\n' "error: $*" >&2; exit 1; }

has curl || die "curl required"
has sudo || die "sudo required"
has dnf || die "dnf required"

url="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
  | grep -Eo 'https://[^" ]+sandevistan-[^" ]+-x86\.rpm' \
  | head -n1)"

[ -n "$url" ] || die "sandevistan .rpm release asset not found"

workdir="$(mktemp -d)"
trap 'rm -rf "$workdir"' EXIT

printf '%s\n' "downloading latest .rpm release"
curl -fsSL "$url" -o "$workdir/sandevistan.rpm"

printf '%s\n' "installing sandevistan"
sudo dnf install -y "$workdir/sandevistan.rpm"

printf '%s\n' "installed native package: sandevistan"
printf '%s\n' "run: sandevistan"
