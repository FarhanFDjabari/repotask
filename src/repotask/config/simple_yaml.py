"""Small YAML subset reader/writer for RepoTask configuration files."""

from __future__ import annotations

import json
from typing import Any


def load_yaml(content: str) -> dict[str, Any]:
    root: dict[str, Any] = {}
    stack: list[tuple[int, Any]] = [(-1, root)]
    lines = content.splitlines()
    for line_number, raw_line in enumerate(lines, start=1):
        if not raw_line.strip() or raw_line.lstrip().startswith("#"):
            continue
        if raw_line.startswith("\t"):
            raise ValueError(f"tabs are not supported at line {line_number}")
        indent = len(raw_line) - len(raw_line.lstrip(" "))
        stripped = raw_line.strip()
        if stripped.startswith("- "):
            # Block sequence items may align with their key (same indent) or be
            # indented deeper, so only discard containers nested past this item.
            while stack and stack[-1][0] > indent:
                stack.pop()
            if not stack or not isinstance(stack[-1][1], list):
                raise ValueError(f"list item without list parent at line {line_number}")
            stack[-1][1].append(_parse_scalar(stripped[2:].strip(), line_number))
            continue
        while stack and indent <= stack[-1][0]:
            stack.pop()
        if not stack:
            raise ValueError(f"invalid indentation at line {line_number}")
        parent = stack[-1][1]
        if ":" not in stripped:
            raise ValueError(f"expected key/value at line {line_number}")
        key, raw_value = stripped.split(":", 1)
        key = key.strip()
        raw_value = raw_value.strip()
        if not key:
            raise ValueError(f"empty key at line {line_number}")
        if not isinstance(parent, dict):
            raise ValueError(f"key/value inside list is not supported at line {line_number}")
        if raw_value:
            parent[key] = _parse_scalar(raw_value, line_number)
            continue
        child = [] if _next_content_line_is_list(lines, line_number, indent) else {}
        parent[key] = child
        stack.append((indent, child))
    return root


def dump_yaml(data: dict[str, Any]) -> str:
    return _dump_mapping(data, 0)


def _dump_mapping(data: dict[str, Any], indent: int) -> str:
    lines: list[str] = []
    prefix = " " * indent
    for key, value in data.items():
        if isinstance(value, dict):
            if value:
                lines.append(f"{prefix}{key}:")
                lines.append(_dump_mapping(value, indent + 2).rstrip("\n"))
            else:
                lines.append(f"{prefix}{key}: {{}}")
        elif isinstance(value, list):
            if value:
                lines.append(f"{prefix}{key}:")
                lines.extend(_dump_list(value, indent + 2))
            else:
                lines.append(f"{prefix}{key}: []")
        else:
            lines.append(f"{prefix}{key}: {_format_scalar(value)}")
    return "\n".join(lines) + "\n"


def _dump_list(values: list[Any], indent: int) -> list[str]:
    prefix = " " * indent
    lines: list[str] = []
    for value in values:
        if isinstance(value, dict):
            lines.append(f"{prefix}-")
            lines.append(_dump_mapping(value, indent + 2).rstrip("\n"))
        elif isinstance(value, list):
            lines.append(f"{prefix}- {_format_scalar(value)}")
        else:
            lines.append(f"{prefix}- {_format_scalar(value)}")
    return lines


def _next_content_line_is_list(
    lines: list[str], current_line_number: int, current_indent: int
) -> bool:
    for raw_line in lines[current_line_number:]:
        if not raw_line.strip() or raw_line.lstrip().startswith("#"):
            continue
        indent = len(raw_line) - len(raw_line.lstrip(" "))
        return indent >= current_indent and raw_line.strip().startswith("- ")
    return False


def _parse_scalar(value: str, line_number: int) -> Any:
    if value in {"[]", "{}"}:
        return [] if value == "[]" else {}
    if value.startswith("["):
        if not value.endswith("]"):
            raise ValueError(f"unterminated inline list at line {line_number}")
        return _parse_inline_list(value, line_number)
    if value.startswith("{"):
        raise ValueError(f"inline mappings are not supported at line {line_number}")
    if (value.startswith('"') and value.endswith('"')) or (
        value.startswith("'") and value.endswith("'")
    ):
        return _parse_quoted(value, line_number)
    if value.endswith("]"):
        raise ValueError(f"unexpected closing delimiter at line {line_number}")
    lowered = value.lower()
    if lowered == "true":
        return True
    if lowered == "false":
        return False
    if lowered in {"null", "~"}:
        return None
    if value.isdigit():
        return int(value)
    return value


def _parse_inline_list(value: str, line_number: int) -> list[Any]:
    inner = value[1:-1].strip()
    if not inner:
        return []
    result: list[Any] = []
    current: list[str] = []
    quote = ""
    escaped = False
    for char in inner:
        if escaped:
            current.append(char)
            escaped = False
            continue
        if quote and char == "\\":
            current.append(char)
            escaped = True
            continue
        if quote:
            current.append(char)
            if char == quote:
                quote = ""
            continue
        if char in {"'", '"'}:
            quote = char
            current.append(char)
            continue
        if char == ",":
            item = "".join(current).strip()
            if not item:
                raise ValueError(f"empty inline list item at line {line_number}")
            result.append(_parse_scalar(item, line_number))
            current = []
            continue
        current.append(char)
    if quote:
        raise ValueError(f"unterminated string at line {line_number}")
    item = "".join(current).strip()
    if not item:
        raise ValueError(f"empty inline list item at line {line_number}")
    result.append(_parse_scalar(item, line_number))
    return result


def _parse_quoted(value: str, line_number: int) -> str:
    if value.startswith('"'):
        try:
            parsed = json.loads(value)
        except json.JSONDecodeError as error:
            raise ValueError(f"invalid quoted string at line {line_number}") from error
        if not isinstance(parsed, str):
            raise ValueError(f"invalid quoted string at line {line_number}")
        return parsed
    return value[1:-1]


def _format_scalar(value: Any) -> str:
    if value is True:
        return "true"
    if value is False:
        return "false"
    if value is None:
        return "null"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, list):
        return "[" + ", ".join(_format_scalar(item) for item in value) + "]"
    text = str(value)
    if _plain_safe(text):
        return text
    return json.dumps(text, ensure_ascii=True)


def _plain_safe(value: str) -> bool:
    if value == "":
        return False
    if value.strip() != value:
        return False
    if value.lower() in {"true", "false", "null", "~"}:
        return False
    if value[0] in "-?:!@#&*[{|>%'\"`":
        return False
    return not any(char in value for char in "\n\r\t")
