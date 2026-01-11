#!/bin/bash
set -euo pipefail

# Configuration
REPO="goudyj/assistant-cli"
BINARY_NAME="assistant"
INSTALL_DIR="${HOME}/.local/bin"
CONFIG_DIR="${HOME}/.config"
CONFIG_FILE="${CONFIG_DIR}/assistant.json"

# Default GitHub Client ID
DEFAULT_GITHUB_CLIENT_ID="Ov23li3PDrRNh2FnCku1"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

info() { echo -e "${GREEN}[INFO]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# Detect OS and architecture
detect_platform() {
    local os arch

    os=$(uname -s | tr '[:upper:]' '[:lower:]')
    arch=$(uname -m)

    case "$os" in
        darwin) os="apple-darwin" ;;
        linux) os="unknown-linux-gnu" ;;
        *) error "Unsupported OS: $os" ;;
    esac

    case "$arch" in
        x86_64|amd64) arch="x86_64" ;;
        arm64|aarch64) arch="aarch64" ;;
        *) error "Unsupported architecture: $arch" ;;
    esac

    echo "${arch}-${os}"
}

# Get latest release version or use specified version
get_version() {
    local version="${1:-latest}"

    if [ "$version" = "latest" ]; then
        version=$(curl -sL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
        if [ -z "$version" ]; then
            error "Failed to fetch latest version. Check your internet connection or specify a version."
        fi
    fi

    echo "$version"
}

# Download and install binary
install_binary() {
    local version="$1"
    local platform="$2"
    local asset_name="assistant-${platform}"
    local download_url="https://github.com/${REPO}/releases/download/${version}/${asset_name}"

    info "Downloading ${asset_name} (${version})..."

    # Create install directory
    mkdir -p "$INSTALL_DIR"

    # Download binary
    local tmp_file
    tmp_file=$(mktemp)
    if ! curl -fsSL "$download_url" -o "$tmp_file"; then
        rm -f "$tmp_file"
        error "Failed to download from ${download_url}"
    fi

    # Make executable and move to install dir
    chmod +x "$tmp_file"
    mv "$tmp_file" "${INSTALL_DIR}/${BINARY_NAME}"

    info "Installed to ${INSTALL_DIR}/${BINARY_NAME}"
}

# Handle macOS quarantine attribute
handle_macos_quarantine() {
    if [ "$(uname -s)" = "Darwin" ]; then
        info "Removing macOS quarantine attribute..."
        xattr -d com.apple.quarantine "${INSTALL_DIR}/${BINARY_NAME}" 2>/dev/null || true
    fi
}

# Create default config file if not exists
create_default_config() {
    if [ -f "$CONFIG_FILE" ]; then
        info "Config file already exists at ${CONFIG_FILE}"
        return
    fi

    info "Creating default config file..."
    mkdir -p "$CONFIG_DIR"

    cat > "$CONFIG_FILE" << EOF
{
  "github_client_id": "${DEFAULT_GITHUB_CLIENT_ID}",
  "projects": {}
}
EOF

    info "Created config at ${CONFIG_FILE}"
}

# Check if PATH includes install dir
check_path() {
    if [[ ":$PATH:" != *":${INSTALL_DIR}:"* ]]; then
        warn "${INSTALL_DIR} is not in your PATH"
        echo ""
        echo "Add this to your shell profile (~/.bashrc, ~/.zshrc, etc.):"
        echo ""
        echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
        echo ""
    fi
}

# Main
main() {
    local version="${1:-latest}"

    info "Installing ${BINARY_NAME}..."

    local platform
    platform=$(detect_platform)
    info "Detected platform: ${platform}"

    version=$(get_version "$version")
    info "Version: ${version}"

    install_binary "$version" "$platform"
    handle_macos_quarantine
    create_default_config
    check_path

    echo ""
    info "Installation complete!"
    info "Run '${BINARY_NAME}' to get started."
}

main "$@"
