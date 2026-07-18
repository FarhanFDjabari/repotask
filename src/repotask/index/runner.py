"""Walk the project and extract declarations into a symbol index."""

from __future__ import annotations

import fnmatch
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any

from repotask.config.models import RepoTaskConfig
from repotask.index.languages import DECLARATIONS, language_for, parser_for
from repotask.output import warn

IDENTIFIER_NODES = {
    "identifier",
    "type_identifier",
    "simple_identifier",
    "field_identifier",
    "property_identifier",
}


@dataclass(frozen=True)
class Symbol:
    name: str
    kind: str
    language: str
    path: str
    line: int
    scope: str = ""


@dataclass
class IndexResult:
    symbols: list[Symbol]
    files_scanned: int
    files_skipped: int
    languages: dict[str, int]

    def as_dict(self) -> dict[str, Any]:
        return {
            "symbols": [asdict(symbol) for symbol in self.symbols],
            "filesScanned": self.files_scanned,
            "filesSkipped": self.files_skipped,
            "languages": self.languages,
        }


def build_index(config: RepoTaskConfig, paths: list[str] | None = None) -> IndexResult:
    """Index the whole project, or only `paths` when given (incremental refresh)."""
    files = _candidate_files(config, paths)
    symbols: list[Symbol] = []
    languages: dict[str, int] = {}
    scanned = 0
    skipped = 0
    missing_grammars: set[str] = set()

    for file in files:
        language = language_for(file.suffix)
        if language is None:
            continue
        if file.stat().st_size > config.index.max_file_bytes:
            skipped += 1
            continue
        parser = parser_for(language)
        if parser is None:
            missing_grammars.add(language)
            skipped += 1
            continue
        relative = str(file.relative_to(config.root))
        found = _extract(parser, language, file.read_bytes(), relative)
        symbols.extend(found)
        scanned += 1
        languages[language] = languages.get(language, 0) + 1

    for language in sorted(missing_grammars):
        warn(f"No tree-sitter grammar available for {language}; those files were skipped.")

    symbols.sort(key=lambda symbol: (symbol.path, symbol.line, symbol.name))
    return IndexResult(symbols, scanned, skipped, languages)


def _candidate_files(config: RepoTaskConfig, paths: list[str] | None) -> list[Path]:
    if paths is not None:
        candidates = [config.root / path for path in paths]
        return [path for path in candidates if path.is_file() and not _excluded(config, path)]
    return [
        path
        for path in sorted(config.root.rglob("*"))
        if path.is_file()
        and path.suffix.lower() in _known_suffixes()
        and not _excluded(config, path)
    ]


def _known_suffixes() -> set[str]:
    from repotask.index.languages import EXTENSIONS

    return set(EXTENSIONS)


def _excluded(config: RepoTaskConfig, path: Path) -> bool:
    relative = str(path.relative_to(config.root))
    if any(part.startswith(".") for part in Path(relative).parts[:-1]):
        return True
    return any(fnmatch.fnmatch(relative, pattern) for pattern in config.index.exclude)


def _extract(parser: Any, language: str, source: bytes, relative: str) -> list[Symbol]:
    declarations = DECLARATIONS.get(language, {})
    if not declarations:
        return []
    tree = parser.parse(source)
    symbols: list[Symbol] = []
    stack: list[tuple[Any, str]] = [(tree.root_node, "")]
    while stack:
        node, scope = stack.pop()
        child_scope = scope
        kind = declarations.get(node.type)
        if kind is not None:
            name = _name_of(node, source)
            if name:
                symbols.append(
                    Symbol(
                        name=name,
                        kind=kind,
                        language=language,
                        path=relative,
                        line=node.start_point[0] + 1,
                        scope=scope,
                    )
                )
                child_scope = f"{scope}.{name}" if scope else name
        for child in reversed(node.children):
            stack.append((child, child_scope))
    return symbols


def _name_of(node: Any, source: bytes) -> str:
    field = node.child_by_field_name("name")
    if field is None:
        # Not every grammar exposes a `name` field — Kotlin uses `simple_identifier`,
        # Dart signatures and Go methods hide the identifier one level down.
        field = next(
            (child for child in node.children if child.type in IDENTIFIER_NODES),
            None,
        )
    if field is None:
        return ""
    return source[field.start_byte : field.end_byte].decode("utf-8", errors="replace")
