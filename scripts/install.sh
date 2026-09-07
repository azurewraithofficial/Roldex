#!/usr/bin/env bash
set -euo pipefail

REPO="azurewraithofficial/Roldex"
INSTALL_DIR="${HOME}/.local/bin"
SKIP_PLUGIN=0
FORCE_CARGO=0

for arg in "$@"; do
  case "$arg" in
    --skip-plugin) SKIP_PLUGIN=1 ;;
    --force-cargo) FORCE_CARGO=1 ;;
    *) echo "Unknown option: $arg" >&2; exit 2 ;;
  esac
done

os="$(uname -s)"
arch="$(uname -m)"
asset=""

case "$os" in
  Linux)
    case "$arch" in
      x86_64|amd64) asset="roldex-linux-x86_64" ;;
    esac
    ;;
  Darwin)
    case "$arch" in
      x86_64|amd64) asset="roldex-macos-x86_64" ;;
      arm64|aarch64) asset="roldex-macos-aarch64" ;;
    esac
    ;;
esac

mkdir -p "$INSTALL_DIR"
installed_binary=0

if [[ "$FORCE_CARGO" -eq 0 && -n "$asset" ]] && command -v curl >/dev/null 2>&1; then
  release_url="https://github.com/${REPO}/releases/latest/download/${asset}"
  echo "Trying latest Roldex release binary..."
  if curl -fL --retry 2 -A "Roldex-Installer" "$release_url" -o "${INSTALL_DIR}/roldex"; then
    chmod +x "${INSTALL_DIR}/roldex"
    installed_binary=1
    echo "Installed Roldex CLI to ${INSTALL_DIR}/roldex"
  else
    rm -f "${INSTALL_DIR}/roldex"
    echo "No compatible release binary was available yet; falling back to Cargo." >&2
  fi
fi

if [[ "$installed_binary" -eq 0 ]]; then
  if ! command -v cargo >/dev/null 2>&1; then
    echo "Roldex needs either a published release binary or Rust/Cargo for the source install." >&2
    echo "Install Rust once with rustup, then run this installer again." >&2
    exit 1
  fi

  echo "Installing Roldex CLI from source with Cargo..."
  cargo install --git "https://github.com/${REPO}" roldex-cli --force
fi

if [[ "$SKIP_PLUGIN" -eq 0 && "$os" == "Darwin" ]]; then
  plugin_dir="${HOME}/Documents/Roblox/Plugins"
  plugin_path="${plugin_dir}/RoldexStudio.plugin.lua"
  plugin_url="https://raw.githubusercontent.com/${REPO}/main/plugins/roldex-studio/RoldexStudio.plugin.lua"
  mkdir -p "$plugin_dir"
  if command -v curl >/dev/null 2>&1; then
    curl -fL --retry 2 -A "Roldex-Installer" "$plugin_url" -o "$plugin_path"
    echo "Installed Roldex Studio plugin to $plugin_path"
    echo "Restart Roblox Studio if it is currently open."
  else
    echo "curl is required to install the Studio plugin automatically." >&2
  fi
fi

case ":${PATH}:" in
  *":${INSTALL_DIR}:"*) ;;
  *)
    echo "Add ${INSTALL_DIR} to PATH if the roldex command is not found."
    ;;
esac

echo ""
echo "Roldex installation complete."
echo "Set OPENROUTER_API_KEY, open your Roblox/Rojo project folder, then run: roldex"
echo "The Studio plugin connects to http://127.0.0.1:38247 by default."
