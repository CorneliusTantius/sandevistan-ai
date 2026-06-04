<div align="center">

<pre style="font-size: 4px; line-height: 1;">
███████╗ █████╗ ███╗   ██╗██████╗ ███████╗██╗   ██╗██╗███████╗████████╗ █████╗ ███╗   ██╗          █████╗ ██╗
██╔════╝██╔══██╗████╗  ██║██╔══██╗██╔════╝██║   ██║██║██╔════╝╚══██╔══╝██╔══██╗████╗  ██║         ██╔══██╗██║
███████╗███████║██╔██╗ ██║██║  ██║█████╗  ██║   ██║██║███████╗   ██║   ███████║██╔██╗ ██║         ███████║██║
╚════██║██╔══██║██║╚██╗██║██║  ██║██╔══╝  ╚██╗ ██╔╝██║╚════██║   ██║   ██╔══██║██║╚██╗██║         ██╔══██║██║
███████║██║  ██║██║ ╚████║██████╔╝███████╗ ╚████╔╝ ██║███████║   ██║   ██║  ██║██║ ╚████║         ██║  ██║██║
╚══════╝╚═╝  ╚═╝╚═╝  ╚═══╝╚═════╝ ╚══════╝  ╚═══╝  ╚═╝╚══════╝   ╚═╝   ╚═╝  ╚═╝╚═╝  ╚═══╝███████╗╚═╝  ╚═╝╚═╝
</pre>

</div>

## Install

| Platform | Command |
|---|---|
| Arch / Linux tar | `curl -fsSL https://github.com/CorneliusTantius/sandevistan-ai/releases/latest/download/sandevistan-1.2.2.tar.gz -o sandevistan.tar.gz && tar -xzf sandevistan.tar.gz && cd sandevistan && ./install.sh` |
| Debian / Ubuntu | `curl -fsSL https://github.com/CorneliusTantius/sandevistan-ai/releases/latest/download/sandevistan-1.2.2-x86.deb -o sandevistan.deb && sudo apt install ./sandevistan.deb` |
| Fedora / RHEL | `curl -fsSL https://github.com/CorneliusTantius/sandevistan-ai/releases/latest/download/sandevistan-1.2.2-x86.rpm -o sandevistan.rpm && sudo dnf install ./sandevistan.rpm` |
| Windows EXE | `winget install --id GitHub.cli && gh release download -R CorneliusTantius/sandevistan-ai -p "sandevistan-*-win.exe" && .\\sandevistan-*-win.exe` |
| Windows MSI | `winget install --id GitHub.cli && gh release download -R CorneliusTantius/sandevistan-ai -p "sandevistan-*-win.msi" && msiexec /i sandevistan-*-win.msi` |
| macOS Intel | `curl -L https://github.com/CorneliusTantius/sandevistan-ai/releases/latest/download/sandevistan-1.2.2-mac-intel.dmg -o sandevistan.dmg && open sandevistan.dmg` |
| macOS Apple Silicon | `curl -L https://github.com/CorneliusTantius/sandevistan-ai/releases/latest/download/sandevistan-1.2.2-mac-arm.dmg -o sandevistan.dmg && open sandevistan.dmg` |

## Tech Stack

- 🦀 Rust
- 🟠 Tauri
- 🧡 Svelte
- 🔷 TypeScript
- ⚡ Vite
- 🟢 Node.js
