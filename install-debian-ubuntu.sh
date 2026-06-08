set -eu

REPO="CorneliusTantius/sandevistan-ai"

has() { command -v "$1" >/dev/null 2>&1; }
die() { printf '%s\n' "error: $*" >&2; exit 1; }

has curl || die "curl required"
has sudo || die "sudo required"
has apt-get || die "apt-get required"

url="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
  | grep -Eo 'https://[^" ]+sandevistan-[^" ]+-x86\.deb' \
  | head -n1)"

[ -n "$url" ] || die "sandevistan .deb release asset not found"

workdir="$(mktemp -d)"
trap 'rm -rf "$workdir"' EXIT

printf '%s\n' "downloading latest .deb release"
curl -fsSL "$url" -o "$workdir/sandevistan.deb"

printf '%s\n' "installing sandevistan"
sudo apt-get update
sudo apt-get install -y "$workdir/sandevistan.deb"

printf '%s\n' "installed native package: sandevistan"
printf '%s\n' "run: sandevistan"
