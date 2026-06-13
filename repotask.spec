# -*- mode: python ; coding: utf-8 -*-

from PyInstaller.utils.hooks import collect_data_files

datas = collect_data_files("repotask", includes=["bundled/**/*"])

a = Analysis(
    ["src/repotask/cli.py"],
    pathex=["src"],
    binaries=[],
    datas=datas,
    hiddenimports=["yaml", "typer", "rich", "prompt_toolkit"],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[],
    noarchive=False,
)
pyz = PYZ(a.pure)
exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.datas,
    [],
    name="repo-task",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    console=True,
)
