#!/bin/bash

# TUIQL Installation Script
# This script automatically detects the platform and downloads the appropriate binary

set -e

VERSION="${VERSION:-0.1.0}"
GITHUB_REPO="tuiql/tuiql"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

# ANSI color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}INFO:${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}WARN:${NC} $1"
}

log_error() {
    echo -e "${RED}ERROR:${NC} $1"
}

# Detect platform and architecture
detect_platform() {
    OS=$(uname -s | tr '[:upper:]' '[:lower:]')
    ARCH=$(uname -m)

    case $OS in
        linux)
            PLATFORM="linux"
            ;;
        darwin)
            PLATFORM="darwin"
            ;;
        *)
            log_error "Unsupported OS: $OS"
            exit 1
            ;;
    esac

    case $ARCH in
        x86_64|amd64)
            ARCH_TYPE="x86_64"
            ;;
        arm64|aarch64)
            ARCH_TYPE="aarch64"
            ;;
        *)
            log_error "Unsupported architecture: $ARCH"
            exit 1
            ;;
    esac

    TARGET="${ARCH_TYPE}-${PLATFORM}"
    log_info "Detected platform: $TARGET"
}

# Download and verify binary
download_binary() {
    local binary_url="https://github.com/${GITHUB_REPO}/releases/download/v${VERSION}/tuiql-v${VERSION}-${TARGET}.tar.gz"
    local temp_dir=$(mktemp -d)
    local archive="${temp_dir}/tuiql.tar.gz"
    local expected_hash=""

    log_info "Downloading TUIQL v${VERSION} for ${TARGET}..."

    if ! curl -L -o "$archive" "$binary_url"; then
        log_error "Failed to download binary from $binary_url"
        rm -rf "$temp_dir"
        exit 1
    fi

    log_info "Download completed. Extracting..."

    if ! tar -xzf "$archive" -C "$temp_dir"; then
        log_error "Failed to extract archive"
        rm -rf "$temp_dir"
        exit 1
    fi

    local binary_path="${temp_dir}/tuiql"
    if [ ! -x "$binary_path" ]; then
        log_error "Binary not found or not executable"
        rm -rf "$temp_dir"
        exit 1
    fi

    # Copy to installation directory
    log_info "Installing to ${INSTALL_DIR}..."
    if [ ! -w "$INSTALL_DIR" ] && [ "$EUID" -ne 0 ]; then
        log_warn "Installation directory is not writable. You may need to run with sudo."
        sudo cp "$binary_path" "$INSTALL_DIR/tuiql"
        sudo chmod +x "$INSTALL_DIR/tuiql"
    else
        cp "$binary_path" "$INSTALL_DIR/tuiql"
        chmod +x "$INSTALL_DIR/tuiql"
    fi

    rm -rf "$temp_dir"
}

# Verify installation
verify_installation() {
    if command -v tuiql >/dev/null 2>&1; then
        log_info "TUIQL installed successfully!"
        tuiql --version
    else
        log_error "Installation verification failed"
        exit 1
    fi
}

# Main installation process
main() {
    log_info "TUIQL Installer v${VERSION}"
    log_info "Repository: ${GITHUB_REPO}"

    detect_platform
    download_binary
    verify_installation

    log_info "Installation completed successfully!"
    log_info "Run 'tuiql --help' to get started."
}

main "$@"