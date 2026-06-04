<div align="center">

<pre style="font-size: 4px; line-height: 1;">
███████╗ █████╗ ███╗   ██╗██████╗ ███████╗██╗   ██╗██╗███████╗████████╗ █████╗ ███╗   ██╗          █████╗ ██╗
██╔════╝██╔══██╗████╗  ██║██╔══██╗██╔════╝██║   ██║██║██╔════╝╚══██╔══╝██╔══██╗████╗  ██║         ██╔══██╗██║
███████╗███████║██╔██╗ ██║██║  ██║█████╗  ██║   ██║██║███████╗   ██║   ███████║██╔██╗ ██║         ███████║██║
╚════██║██╔══██║██║╚██╗██║██║  ██║██╔══╝  ╚██╗ ██╔╝██║╚════██║   ██║   ██╔══██║██║╚██╗██║         ██╔══██║██║
███████║██║  ██║██║ ╚████║██████╔╝███████╗ ╚████╔╝ ██║███████║   ██║   ██║  ██║██║ ╚████║         ██║  ██║██║
╚══════╝╚═╝  ╚═╝╚═╝  ╚═══╝╚═════╝ ╚══════╝  ╚═══╝  ╚═╝╚══════╝   ╚═╝   ╚═╝  ╚═╝╚═╝  ╚═══╝███████╗╚═╝  ╚═╝╚═╝
</pre>
<pre>
███████  █████  ███    ██ ██████  ███████ ██    ██ ██ ███████ ████████  █████  ███    ██          █████  ██ 
██      ██   ██ ████   ██ ██   ██ ██      ██    ██ ██ ██         ██    ██   ██ ████   ██         ██   ██ ██ 
███████ ███████ ██ ██  ██ ██   ██ █████   ██    ██ ██ ███████    ██    ███████ ██ ██  ██         ███████ ██ 
     ██ ██   ██ ██  ██ ██ ██   ██ ██       ██  ██  ██      ██    ██    ██   ██ ██  ██ ██         ██   ██ ██ 
███████ ██   ██ ██   ████ ██████  ███████   ████   ██ ███████    ██    ██   ██ ██   ████ ███████ ██   ██ ██ 
                                                                                                            
                                                                                                            
</pre>

</div>

## Install

| Platform | Command |
|---|---|
| Arch / Linux tar | <code>url=$(curl -fsSL https://api.github.com/repos/CorneliusTantius/sandevistan-ai/releases/latest &#124; grep -Eo 'https://[^\"]+sandevistan-[^\"]+\.tar\.gz' &#124; head -n1) && curl -fsSL "$url" -o sandevistan.tar.gz && tar -xzf sandevistan.tar.gz && cd sandevistan && ./install.sh</code> |
| Debian / Ubuntu | <code>url=$(curl -fsSL https://api.github.com/repos/CorneliusTantius/sandevistan-ai/releases/latest &#124; grep -Eo 'https://[^\"]+sandevistan-[^\"]+-x86\.deb' &#124; head -n1) && curl -fsSL "$url" -o sandevistan.deb && sudo apt install ./sandevistan.deb</code> |
| Fedora / RHEL | <code>url=$(curl -fsSL https://api.github.com/repos/CorneliusTantius/sandevistan-ai/releases/latest &#124; grep -Eo 'https://[^\"]+sandevistan-[^\"]+-x86\.rpm' &#124; head -n1) && curl -fsSL "$url" -o sandevistan.rpm && sudo dnf install ./sandevistan.rpm</code> |
| Windows EXE | <code>winget install --id GitHub.cli && gh release download -R CorneliusTantius/sandevistan-ai -p "sandevistan-*-win.exe" && .\sandevistan-*-win.exe</code> |
| Windows MSI | <code>winget install --id GitHub.cli && gh release download -R CorneliusTantius/sandevistan-ai -p "sandevistan-*-win.msi" && msiexec /i sandevistan-*-win.msi</code> |
| macOS Intel | <code>url=$(curl -fsSL https://api.github.com/repos/CorneliusTantius/sandevistan-ai/releases/latest &#124; grep -Eo 'https://[^\"]+sandevistan-[^\"]+-mac-intel\.dmg' &#124; head -n1) && curl -L "$url" -o sandevistan.dmg && open sandevistan.dmg</code> |
| macOS Apple Silicon | <code>url=$(curl -fsSL https://api.github.com/repos/CorneliusTantius/sandevistan-ai/releases/latest &#124; grep -Eo 'https://[^\"]+sandevistan-[^\"]+-mac-arm\.dmg' &#124; head -n1) && curl -L "$url" -o sandevistan.dmg && open sandevistan.dmg</code> |

## Tech Stack

- 🦀 Rust
- 🟠 Tauri
- 🧡 Svelte
- 🔷 TypeScript
- ⚡ Vite
- 🟢 Node.js
