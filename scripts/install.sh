#!/usr/bin/env bash
# ==============================================================================
# Auralis One-Line Installer
# Installs the verified Auralis binary + bundled TimGM6mb.sf2 SoundFont.
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/Celestrionx/auralis/main/scripts/install.sh | bash
# Or specific tag:
#   curl -fsSL https://raw.githubusercontent.com/Celestrionx/auralis/main/scripts/install.sh | bash -s -- v0.2.0
# ==============================================================================

set -euo pipefail

REPO="Celestrionx/auralis"
TARGET_VERSION="${1:-}"

info() { printf '\033[0;34m==>\033[0m %s\n' "$1"; }
success() { printf '\033[0;32m==>\033[0m \033[1m%s\033[0m\n' "$1"; }
warn() { printf '\033[1;33mWarning:\033[0m %s\n' "$1"; }
error() { printf '\033[0;31mError:\033[0m %s\n' "$1" >&2; exit 1; }

OS_RAW="$(uname -s)"
case "${OS_RAW}" in
  Linux*)  OS="linux" ;;
  Darwin*) OS="darwin" ;;
  *) error "Unsupported OS for bash installer: ${OS_RAW}. Download release zip on Windows." ;;
esac

ARCH_RAW="$(uname -m)"
case "${ARCH_RAW}" in
  x86_64|amd64)  ARCH="amd64" ;;
  arm64|aarch64) ARCH="arm64" ;;
  *) error "Unsupported architecture: ${ARCH_RAW}" ;;
esac

ARCHIVE_NAME="auralis-${OS}-${ARCH}.tar.gz"

if [ -z "${TARGET_VERSION}" ]; then
  info "Resolving latest release of Auralis..."
  TARGET_VERSION="$(curl -sI "https://github.com/${REPO}/releases/latest" 2>/dev/null | grep -i "^location:" | sed -e 's/.*tag\///' | tr -d '\r\n' || true)"
  if [ -z "${TARGET_VERSION}" ]; then
    TARGET_VERSION="$(curl -sSL "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/' || true)"
  fi
  [ -n "${TARGET_VERSION}" ] || error "Could not resolve latest release. Pass tag explicitly or install via: cargo install --git https://github.com/${REPO}"
fi

TMP_DIR="$(mktemp -d -t auralis-install-XXXXXX)"
trap 'rm -rf "${TMP_DIR}"' EXIT

ARCHIVE_URL="https://github.com/${REPO}/releases/download/${TARGET_VERSION}/${ARCHIVE_NAME}"
CHECKSUM_URL="https://github.com/${REPO}/releases/download/${TARGET_VERSION}/checksums.txt"

info "Downloading ${ARCHIVE_NAME} (${TARGET_VERSION})..."
curl -sSL --fail "${ARCHIVE_URL}" -o "${TMP_DIR}/${ARCHIVE_NAME}" || error "Download failed: ${ARCHIVE_URL}"

if curl -sSL --fail "${CHECKSUM_URL}" -o "${TMP_DIR}/checksums.txt"; then
  EXPECTED_SHA="$(grep -F "${ARCHIVE_NAME}" "${TMP_DIR}/checksums.txt" | awk '{print $1}' || true)"
  if [ -n "${EXPECTED_SHA}" ]; then
    if command -v sha256sum >/dev/null 2>&1; then
      ACTUAL_SHA="$(sha256sum "${TMP_DIR}/${ARCHIVE_NAME}" | awk '{print $1}')"
    elif command -v shasum >/dev/null 2>&1; then
      ACTUAL_SHA="$(shasum -a 256 "${TMP_DIR}/${ARCHIVE_NAME}" | awk '{print $1}')"
    else
      ACTUAL_SHA=""
    fi
    if [ -n "${ACTUAL_SHA}" ] && [ "${EXPECTED_SHA}" != "${ACTUAL_SHA}" ]; then
      error "SHA-256 mismatch! Expected ${EXPECTED_SHA}, got ${ACTUAL_SHA}"
    fi
    [ -n "${ACTUAL_SHA}" ] && info "SHA-256 verified (${ACTUAL_SHA:0:16}...)"
  fi
else
  warn "checksums.txt unavailable; skipping checksum verification."
fi

tar -C "${TMP_DIR}" -xzf "${TMP_DIR}/${ARCHIVE_NAME}"

INSTALL_DIR="${HOME}/.local/bin"
if [ -w "/usr/local/bin" ]; then
  INSTALL_DIR="/usr/local/bin"
fi
mkdir -p "${INSTALL_DIR}"

if [ -w "${INSTALL_DIR}" ]; then
  mv "${TMP_DIR}/auralis" "${INSTALL_DIR}/auralis"
else
  sudo mv "${TMP_DIR}/auralis" "${INSTALL_DIR}/auralis"
fi
chmod +x "${INSTALL_DIR}/auralis"

DATA_DIR="${XDG_DATA_HOME:-${HOME}/.local/share}/auralis/assets/soundfonts"
mkdir -p "${DATA_DIR}"
if [ -f "${TMP_DIR}/assets/soundfonts/TimGM6mb.sf2" ]; then
  cp "${TMP_DIR}/assets/soundfonts/TimGM6mb.sf2" "${DATA_DIR}/TimGM6mb.sf2"
  info "Installed bundled SoundFont to ${DATA_DIR}/TimGM6mb.sf2"
fi

success "Auralis ${TARGET_VERSION} installed to ${INSTALL_DIR}/auralis"
if ! command -v auralis >/dev/null 2>&1; then
  warn "Add ${INSTALL_DIR} to your PATH: export PATH=\"${INSTALL_DIR}:\$PATH\""
fi
printf '\nMCP client config:\n  {"mcpServers":{"auralis":{"command":"auralis","args":["serve"]}}}\n'
