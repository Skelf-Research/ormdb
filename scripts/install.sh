#!/bin/bash

# ORMDB Installation Script
# Downloads pre-compiled binaries from GitHub releases or builds from source
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/Skelf-Research/ormdb/main/scripts/install.sh | bash
#   ./install.sh --version 0.1.0 --dir /usr/local/bin
#   ./install.sh --from-source

set -e
set -o pipefail

# ============================================
# Configuration
# ============================================
REPO="Skelf-Research/ormdb"
INSTALL_DIR="${ORMDB_INSTALL_DIR:-$HOME/.ormdb/bin}"
BINARIES=("ormdb-server" "ormdb" "ormdb-gateway" "ormdb-studio")

# Colors (disabled if not a terminal)
if [[ -t 1 ]]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[1;33m'
    BLUE='\033[0;34m'
    BOLD='\033[1m'
    NC='\033[0m'
else
    RED=''
    GREEN=''
    YELLOW=''
    BLUE=''
    BOLD=''
    NC=''
fi

# ============================================
# Helper Functions
# ============================================

info() {
    echo -e "${BLUE}==>${NC} ${BOLD}$1${NC}"
}

success() {
    echo -e "${GREEN}==>${NC} $1"
}

warning() {
    echo -e "${YELLOW}Warning:${NC} $1"
}

error() {
    echo -e "${RED}Error:${NC} $1" >&2
    exit "${2:-1}"
}

show_help() {
    cat <<EOF
ORMDB Installation Script

Usage:
    install.sh [OPTIONS]

Options:
    -v, --version VERSION    Install specific version (default: latest)
    -d, --dir DIR           Installation directory (default: ~/.ormdb/bin)
    --skip-path             Don't modify shell PATH configuration
    --from-source           Force building from source instead of downloading
    -h, --help              Show this help message

Environment Variables:
    ORMDB_INSTALL_DIR       Override default installation directory

Examples:
    # Install latest version
    ./install.sh

    # Install specific version
    ./install.sh --version 0.1.0

    # Install to custom directory
    ./install.sh --dir /usr/local/bin

    # Build from source
    ./install.sh --from-source

    # One-liner installation
    curl -fsSL https://raw.githubusercontent.com/Skelf-Research/ormdb/main/scripts/install.sh | bash
EOF
}

detect_platform() {
    local platform
    case "$(uname -s)" in
        Linux*)
            platform="linux"
            ;;
        Darwin*)
            platform="darwin"
            ;;
        CYGWIN*|MINGW*|MSYS*)
            platform="windows"
            ;;
        *)
            error "Unsupported platform: $(uname -s)" 1
            ;;
    esac
    echo "$platform"
}

detect_architecture() {
    local arch
    case "$(uname -m)" in
        x86_64|amd64)
            arch="x86_64"
            ;;
        aarch64|arm64)
            arch="aarch64"
            ;;
        *)
            error "Unsupported architecture: $(uname -m)" 1
            ;;
    esac
    echo "$arch"
}

require_command() {
    if ! command -v "$1" &>/dev/null; then
        error "$1 is required but not installed" 1
    fi
}

get_latest_version() {
    local version

    if command -v curl &>/dev/null; then
        version=$(curl -sL "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null | \
            grep '"tag_name":' | sed -E 's/.*"v([^"]+)".*/\1/')
    elif command -v wget &>/dev/null; then
        version=$(wget -qO- "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null | \
            grep '"tag_name":' | sed -E 's/.*"v([^"]+)".*/\1/')
    else
        error "curl or wget is required" 1
    fi

    if [[ -z "$version" ]]; then
        return 1
    fi

    echo "$version"
}

download_with_retry() {
    local url="$1"
    local output="$2"
    local max_retries=3
    local retry_delay=3

    for i in $(seq 1 $max_retries); do
        if command -v curl &>/dev/null; then
            if curl -fsSL "$url" -o "$output" 2>/dev/null; then
                return 0
            fi
        elif command -v wget &>/dev/null; then
            if wget -q "$url" -O "$output" 2>/dev/null; then
                return 0
            fi
        fi

        if [[ $i -lt $max_retries ]]; then
            warning "Download failed (attempt $i/$max_retries). Retrying in ${retry_delay}s..."
            sleep "$retry_delay"
        fi
    done

    return 1
}

download_binary() {
    local version="$1"
    local platform="$2"
    local arch="$3"
    local ext="tar.gz"

    [[ "$platform" == "windows" ]] && ext="zip"

    local filename="ormdb-v${version}-${platform}-${arch}.${ext}"
    local url="https://github.com/${REPO}/releases/download/v${version}/${filename}"
    local tmp_file="/tmp/${filename}"

    info "Downloading ORMDB v${version} for ${platform}-${arch}..."
    echo "    URL: $url"

    if ! download_with_retry "$url" "$tmp_file"; then
        rm -f "$tmp_file"
        return 1
    fi

    echo "$tmp_file"
}

install_binaries() {
    local archive="$1"
    local install_dir="$2"

    info "Installing binaries to ${install_dir}..."

    mkdir -p "$install_dir"

    if [[ "$archive" == *.zip ]]; then
        if ! command -v unzip &>/dev/null; then
            error "unzip is required to extract Windows archives" 1
        fi
        unzip -o "$archive" -d "$install_dir"
    else
        tar -xzf "$archive" -C "$install_dir"
    fi

    # Make binaries executable
    for bin in "${BINARIES[@]}"; do
        local bin_path="${install_dir}/${bin}"
        if [[ -f "$bin_path" ]]; then
            chmod +x "$bin_path"
            success "Installed: $bin"
        fi
    done
}

install_from_source() {
    info "Building from source..."

    if ! command -v cargo &>/dev/null; then
        echo ""
        error "Rust toolchain not found.

To install Rust, run:
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

Then re-run this script." 1
    fi

    local cargo_version
    cargo_version=$(cargo --version)
    echo "    Using: $cargo_version"
    echo ""

    info "Installing ORMDB binaries via cargo..."
    echo "    This may take a few minutes..."
    echo ""

    cargo install ormdb-cli --locked
    cargo install ormdb-server --locked
    cargo install ormdb-gateway --locked
    cargo install ormdb-studio --locked

    local cargo_bin="${CARGO_HOME:-$HOME/.cargo}/bin"
    success "Binaries installed to: $cargo_bin"

    # Return the cargo bin path so we know not to modify PATH
    echo "$cargo_bin"
}

add_to_path() {
    local install_dir="$1"
    local shell_rc=""
    local shell_name=""

    # Don't add cargo bin to PATH - it's already there
    if [[ "$install_dir" == *".cargo/bin"* ]]; then
        return 0
    fi

    # Check if already in PATH
    if echo "$PATH" | tr ':' '\n' | grep -qx "$install_dir"; then
        info "Installation directory already in PATH"
        return 0
    fi

    # Detect shell configuration file
    case "${SHELL:-/bin/bash}" in
        */zsh)
            shell_name="zsh"
            shell_rc="$HOME/.zshrc"
            ;;
        */bash)
            shell_name="bash"
            if [[ -f "$HOME/.bash_profile" ]]; then
                shell_rc="$HOME/.bash_profile"
            else
                shell_rc="$HOME/.bashrc"
            fi
            ;;
        */fish)
            shell_name="fish"
            shell_rc="$HOME/.config/fish/config.fish"
            ;;
        *)
            shell_name="sh"
            shell_rc="$HOME/.profile"
            ;;
    esac

    # Check if already configured
    if grep -q "$install_dir" "$shell_rc" 2>/dev/null; then
        info "PATH already configured in $shell_rc"
        return 0
    fi

    info "Adding ORMDB to PATH in $shell_rc..."

    {
        echo ""
        echo "# ORMDB"
        if [[ "$shell_name" == "fish" ]]; then
            echo "set -gx PATH \"${install_dir}\" \$PATH"
        else
            echo "export PATH=\"${install_dir}:\$PATH\""
        fi
    } >> "$shell_rc"

    success "Updated $shell_rc"
    echo ""
    echo "    To use ORMDB now, run:"
    echo "        source $shell_rc"
    echo ""
    echo "    Or open a new terminal window."
}

verify_installation() {
    local install_dir="$1"
    local found=0
    local total=${#BINARIES[@]}

    echo ""
    info "Verifying installation..."

    for bin in "${BINARIES[@]}"; do
        local bin_path="${install_dir}/${bin}"
        if [[ -x "$bin_path" ]]; then
            local version_output
            version_output=$("$bin_path" --version 2>/dev/null || echo "unknown")
            echo "    $bin: $version_output"
            ((found++))
        else
            # Check in PATH
            if command -v "$bin" &>/dev/null; then
                local version_output
                version_output=$("$bin" --version 2>/dev/null || echo "unknown")
                echo "    $bin: $version_output (in PATH)"
                ((found++))
            else
                warning "$bin not found"
            fi
        fi
    done

    echo ""
    if [[ $found -eq $total ]]; then
        success "All $total binaries installed successfully!"
    else
        success "$found of $total binaries installed"
    fi
}

cleanup() {
    local exit_code=$?
    rm -f /tmp/ormdb-*.tar.gz /tmp/ormdb-*.zip 2>/dev/null || true
    exit $exit_code
}

# ============================================
# Main
# ============================================

main() {
    local version=""
    local install_dir="$INSTALL_DIR"
    local skip_path=false
    local force_source=false

    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            -v|--version)
                version="$2"
                shift 2
                ;;
            -d|--dir)
                install_dir="$2"
                shift 2
                ;;
            --skip-path)
                skip_path=true
                shift
                ;;
            --from-source)
                force_source=true
                shift
                ;;
            -h|--help)
                show_help
                exit 0
                ;;
            *)
                error "Unknown option: $1. Use --help for usage information." 1
                ;;
        esac
    done

    # Set up cleanup trap
    trap cleanup EXIT

    echo ""
    echo "========================================"
    echo "       ORMDB Installation Script"
    echo "========================================"
    echo ""

    local platform
    local arch
    platform=$(detect_platform)
    arch=$(detect_architecture)

    # Get version if not specified
    if [[ -z "$version" ]]; then
        info "Fetching latest release version..."
        if ! version=$(get_latest_version); then
            warning "Could not fetch latest version from GitHub"
            if [[ "$force_source" != "true" ]]; then
                echo "    Falling back to source build..."
                force_source=true
            fi
        fi
    fi

    echo ""
    echo "    Version:      v${version:-latest}"
    echo "    Platform:     ${platform}-${arch}"
    echo "    Install dir:  ${install_dir}"
    echo ""

    if [[ "$force_source" == "true" ]]; then
        local cargo_bin
        cargo_bin=$(install_from_source)
        install_dir="$cargo_bin"
    else
        local archive
        if archive=$(download_binary "$version" "$platform" "$arch"); then
            install_binaries "$archive" "$install_dir"
            rm -f "$archive"
        else
            echo ""
            warning "Pre-compiled binaries not available for ${platform}-${arch}"
            echo "    Falling back to source build..."
            echo ""

            local cargo_bin
            cargo_bin=$(install_from_source)
            install_dir="$cargo_bin"
        fi
    fi

    if [[ "$skip_path" != "true" ]]; then
        add_to_path "$install_dir"
    fi

    verify_installation "$install_dir"

    echo ""
    echo "========================================"
    echo "    Installation Complete!"
    echo "========================================"
    echo ""
    echo "Quick start:"
    echo "    ormdb-server --port 9000    # Start database server"
    echo "    ormdb                       # Start CLI REPL"
    echo ""
    echo "Documentation: https://docs.skelfresearch.com/ormdb"
    echo ""
}

main "$@"
