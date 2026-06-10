"""Technology stack recommendations."""

from __future__ import annotations

import json
from pathlib import Path


def detect_stacks(root: Path) -> list[str]:
    stacks: list[str] = []

    def add(*values: str) -> None:
        for value in values:
            if value not in stacks:
                stacks.append(value)

    names = {path.name for path in root.iterdir()}
    if any(name in names for name in {"build.gradle", "build.gradle.kts", "settings.gradle.kts"}):
        add("android", "kotlin")
        gradle_text = "\n".join(
            path.read_text(encoding="utf-8", errors="ignore")
            for path in root.glob("**/*.gradle.kts")
            if ".git" not in path.parts
        )
        if "compose" in gradle_text.lower():
            add("jetpack-compose")
    if any(root.glob("*.xcodeproj")) or any(root.glob("*.xcworkspace")):
        add("ios", "swift")
    if (root / "Package.swift").exists():
        add("swift")
    if (root / "pubspec.yaml").exists():
        add("flutter", "dart")
    package_json = root / "package.json"
    if package_json.exists():
        add("typescript" if (root / "tsconfig.json").exists() else "web")
        try:
            package = json.loads(package_json.read_text(encoding="utf-8"))
            dependencies = {**package.get("dependencies", {}), **package.get("devDependencies", {})}
            if "react-native" in dependencies:
                add("react-native", "typescript")
            else:
                add("web")
        except (json.JSONDecodeError, OSError):
            add("web")
    if (root / "pyproject.toml").exists() or (root / "requirements.txt").exists():
        add("python")
    if (root / "go.mod").exists():
        add("go")
    if (root / "Cargo.toml").exists():
        add("rust")
    return stacks or ["generic"]

