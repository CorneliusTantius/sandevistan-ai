<div align="center">

<pre style="line-height: 1;">
  ____    _    _   _ ____  _______     _____ ____ _____  _    _   _      _    ___ 
 / ___|  / \  | \ | |  _ \| ____\ \   / /_ _/ ___|_   _|/ \  | \ | |    / \  |_ _|
 \___ \ / _ \ |  \| | | | |  _|  \ \ / / | |\___ \ | | / _ \ |  \| |   / _ \  | | 
  ___) / ___ \| |\  | |_| | |___  \ V /  | | ___) || |/ ___ \| |\  |_ / ___ \ | | 
 |____/_/   \_\_| \_|____/|_____|  \_/  |___|____/ |_/_/   \_\_| \_(_)_/   \_\___|
                                                                                  
</pre>

</div>

## Install

| Platform | Command |
|---|---|
| Arch / Linux tar | <code>curl -fsSL https://raw.githubusercontent.com/CorneliusTantius/sandevistan-ai/main/docs/scripts/install-arch-native.sh &#124; sh</code> |
| Debian / Ubuntu | <code>url=$(curl -fsSL https://api.github.com/repos/CorneliusTantius/sandevistan-ai/releases/latest &#124; grep -Eo 'https://[^\"]+sandevistan-[^\"]+-x86\.deb' &#124; head -n1) && curl -fsSL "$url" -o sandevistan.deb && sudo apt install ./sandevistan.deb</code> |
| Fedora / RHEL | <code>url=$(curl -fsSL https://api.github.com/repos/CorneliusTantius/sandevistan-ai/releases/latest &#124; grep -Eo 'https://[^\"]+sandevistan-[^\"]+-x86\.rpm' &#124; head -n1) && curl -fsSL "$url" -o sandevistan.rpm && sudo dnf install ./sandevistan.rpm</code> |
| Windows | Download installer from latest release. |
| macOS Intel / Apple Silicon | <code>curl -fsSL https://raw.githubusercontent.com/CorneliusTantius/sandevistan-ai/main/docs/scripts/install-macos.sh &#124; sh</code> |

<!-- macOS installer removes quarantine automatically. -->

## Tech Stack

- 🦀 Rust
- 🟠 Tauri
- 🧡 Svelte
- 🔷 TypeScript
- ⚡ Vite
- 🟢 Node.js

## Installation scripts

Use the matching helper script for your OS/distro:

```bash
# Arch Linux
./install-arch.sh

# macOS
./install-macos.sh

# Debian / Ubuntu
./install-debian-ubuntu.sh

# Fedora
./install-fedora.sh
```

If needed, make scripts executable first:

```bash
chmod +x install-*.sh
```

