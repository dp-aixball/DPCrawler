@echo off
REM Build the Python crawler as a standalone executable using PyInstaller (Windows)
REM Output goes to src-tauri\binaries\ for Tauri sidecar bundling

setlocal EnableDelayedExpansion

cd /d "%~dp0"
set "PROJECT_ROOT=%CD%"
set "PYTHON=%PROJECT_ROOT%\.venv\Scripts\python.exe"
set "PYINSTALLER=%PROJECT_ROOT%\.venv\Scripts\pyinstaller.exe"

echo === Building crawler sidecar for Windows ===

REM Ensure PyInstaller is installed
if not exist "%PYINSTALLER%" (
    echo Installing PyInstaller...
    "%PYTHON%" -m pip install pyinstaller -q
)

REM Detect architecture
set "ARCH=%PROCESSOR_ARCHITECTURE%"
if "%ARCH%"=="AMD64" (
    set "RUST_ARCH=x86_64"
) else if "%ARCH%"=="ARM64" (
    set "RUST_ARCH=aarch64"
) else (
    set "RUST_ARCH=%ARCH%"
)

set "TARGET_TRIPLE=%RUST_ARCH%-pc-windows-msvc"
echo Target: %TARGET_TRIPLE%

REM Build with PyInstaller
cd "%PROJECT_ROOT%\python"
"%PYINSTALLER%" ^
    --onefile ^
    --clean ^
    --noconfirm ^
    --distpath "%PROJECT_ROOT%\src-tauri\binaries" ^
    --workpath "%PROJECT_ROOT%\python\build" ^
    --specpath "%PROJECT_ROOT%\python" ^
    --name "crawler-%TARGET_TRIPLE%.exe" ^
    --paths "%PROJECT_ROOT%\python" ^
    crawler.py ^
    --hidden-import config ^
    --hidden-import storage ^
    --hidden-import pdfplumber ^
    --hidden-import docx ^
    --hidden-import openpyxl ^
    --hidden-import pptx ^
    --hidden-import html2text ^
    --hidden-import bs4 ^
    --hidden-import yaml ^
    --hidden-import requests ^
    --exclude-module tkinter ^
    --exclude-module matplotlib ^
    --exclude-module numpy

echo.
echo === Build complete ===
echo Sidecar binary: src-tauri\binaries\crawler-%TARGET_TRIPLE%.exe
dir "%PROJECT_ROOT%\src-tauri\binaries\crawler-%TARGET_TRIPLE%.exe"
