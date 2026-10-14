#!/bin/sh
# Installs the memelint release binaries into ~/.local/bin, or $MEMELINT_INSTALL_DIR.
set -eu

case "$(uname -m)" in x86_64) arch=x86_64 ;; arm64 | aarch64) arch=aarch64 ;; esac
case "$(uname -s)" in Linux) os=unknown-linux-musl ;; Darwin) os=apple-darwin ;; esac
dir="${MEMELINT_INSTALL_DIR:-$HOME/.local/bin}"
url="https://github.com/antimemes/memelint/releases/latest/download/memelint-${arch:?no release for $(uname -m)}-${os:?no release for $(uname -s)}.tar.gz"

mkdir -p "$dir"
curl -fsSL "$url" | tar -xz -C "$dir"
echo "installed $dir/memelint"
