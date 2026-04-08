# -*- mode: python ; coding: utf-8 -*-
# PyInstaller spec for DPCrawler crawler sidecar

import sys
import os

block_cipher = None

a = Analysis(
    ['crawler.py'],
    pathex=[],
    binaries=[],
    datas=[],
    hiddenimports=[
        'config',
        'storage',
        'pdfplumber',
        'docx',
        'openpyxl',
        'pptx',
        'html2text',
        'bs4',
        'yaml',
        'requests',
    ],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=['tkinter', 'matplotlib', 'numpy', 'scipy', 'PIL'],
    win_no_prefer_redirects=False,
    win_private_assemblies=False,
    cipher=block_cipher,
    noarchive=False,
)

pyz = PYZ(a.pure, a.zipped_data, cipher=block_cipher)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.datas,
    [],
    name='crawler',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    upx_exclude=[],
    runtime_tmpdir=None,
    console=True,
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
)
