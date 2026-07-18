"""Language detection and the node types that count as declarations.

Symbol extraction stays deliberately generic: a declaration is any node whose
type is mapped below, and its name is the node's `name` field. Anything more
specific belongs in a fact family, not here.
"""

from __future__ import annotations

from functools import cache
from typing import Any

EXTENSIONS = {
    ".kt": "kotlin",
    ".kts": "kotlin",
    ".java": "java",
    ".swift": "swift",
    ".dart": "dart",
    ".ts": "typescript",
    ".tsx": "tsx",
    ".js": "javascript",
    ".jsx": "javascript",
    ".py": "python",
    ".go": "go",
    ".rs": "rust",
}

DECLARATIONS: dict[str, dict[str, str]] = {
    "kotlin": {
        "class_declaration": "class",
        "object_declaration": "object",
        "function_declaration": "function",
        "property_declaration": "property",
    },
    "java": {
        "class_declaration": "class",
        "interface_declaration": "interface",
        "enum_declaration": "enum",
        "method_declaration": "function",
    },
    "swift": {
        "class_declaration": "class",
        "protocol_declaration": "protocol",
        "function_declaration": "function",
        "property_declaration": "property",
    },
    "dart": {
        "class_definition": "class",
        "mixin_declaration": "mixin",
        "enum_declaration": "enum",
        "function_signature": "function",
    },
    "typescript": {
        "class_declaration": "class",
        "interface_declaration": "interface",
        "enum_declaration": "enum",
        "function_declaration": "function",
        "type_alias_declaration": "type",
    },
    "python": {
        "class_definition": "class",
        "function_definition": "function",
    },
    "go": {
        "type_declaration": "type",
        "function_declaration": "function",
        "method_declaration": "function",
    },
    "rust": {
        "struct_item": "struct",
        "enum_item": "enum",
        "trait_item": "trait",
        "function_item": "function",
        "impl_item": "impl",
    },
}
DECLARATIONS["tsx"] = DECLARATIONS["typescript"]
DECLARATIONS["javascript"] = {
    key: value
    for key, value in DECLARATIONS["typescript"].items()
    if key in {"class_declaration", "function_declaration"}
}


def language_for(suffix: str) -> str | None:
    return EXTENSIONS.get(suffix.lower())


@cache
def parser_for(language: str) -> Any | None:
    """Return a tree-sitter parser, or None when the grammar is unavailable."""
    try:
        from tree_sitter_language_pack import get_parser

        return get_parser(language)  # type: ignore[arg-type]
    except (ImportError, LookupError, AttributeError):
        return None
