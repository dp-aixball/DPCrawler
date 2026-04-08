#!/bin/bash
# Build the Python crawler as a standalone executable using PyInstaller
# Output goes to src-tauri/binaries/ for Tauri sidecar bundling

set -e

PROJECT_ROOT="$(cd "$(dirname "$0")" && pwd)"
PYTHON="${PROJECT_ROOT}/.venv/bin/python3"
PYINSTALLER="${PROJECT_ROOT}/.venv/bin/pyinstaller"

echo "=== Building crawler sidecar ==="

# Ensure PyInstaller is installed
if [ ! -f "$PYINSTALLER" ]; then
    echo "Installing PyInstaller..."
    "$PYTHON" -m pip install pyinstaller -q
fi

# Detect target triple for Tauri sidecar naming
ARCH=$(uname -m)
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
case "$ARCH" in
    x86_64)  RUST_ARCH="x86_64" ;;
    aarch64) RUST_ARCH="aarch64" ;;
    arm64)   RUST_ARCH="aarch64" ;;
    *)       RUST_ARCH="$ARCH" ;;
esac
case "$OS" in
    linux)  TARGET_TRIPLE="${RUST_ARCH}-unknown-linux-gnu" ;;
    darwin) TARGET_TRIPLE="${RUST_ARCH}-apple-darwin" ;;
    *)      TARGET_TRIPLE="${RUST_ARCH}-pc-windows-msvc" ;;
esac

echo "Target: $TARGET_TRIPLE"

# Build with PyInstaller
cd "${PROJECT_ROOT}/python"
"$PYINSTALLER" \
    --onefile \
    --clean \
    --noconfirm \
    --distpath "${PROJECT_ROOT}/src-tauri/binaries" \
    --workpath "${PROJECT_ROOT}/python/build" \
    --specpath "${PROJECT_ROOT}/python" \
    --name "crawler-${TARGET_TRIPLE}" \
    --paths "${PROJECT_ROOT}/python" \
    crawler.py \
    --hidden-import config \
    --hidden-import storage \
    --hidden-import pdfplumber \
    --hidden-import docx \
    --hidden-import openpyxl \
    --hidden-import pptx \
    --hidden-import html2text \
    --hidden-import bs4 \
    --hidden-import yaml \
    --hidden-import requests \
    --exclude-module tkinter \
    --exclude-module matplotlib \
    --exclude-module numpy

echo ""
echo "=== Build complete ==="
echo "Sidecar binary: src-tauri/binaries/crawler-${TARGET_TRIPLE}"
ls -lh "${PROJECT_ROOT}/src-tauri/binaries/crawler-${TARGET_TRIPLE}"
