//! Language detection and the node types that count as declarations.
//!
//! Symbol extraction stays deliberately generic: a declaration is any node whose
//! type is mapped below, and its name is the node's `name` field. Anything more
//! specific belongs in a fact family, not here.
//!
//! Grammars are behind cargo features so a size-constrained build can ship a subset.

use tree_sitter::Language;

pub fn language_for(suffix: &str) -> Option<&'static str> {
    Some(match suffix.to_ascii_lowercase().as_str() {
        "kt" | "kts" => "kotlin",
        "java" => "java",
        "swift" => "swift",
        "dart" => "dart",
        "ts" => "typescript",
        "tsx" => "tsx",
        "js" | "jsx" => "javascript",
        "py" => "python",
        "go" => "go",
        "rs" => "rust",
        _ => return None,
    })
}

pub fn is_known_suffix(suffix: &str) -> bool {
    language_for(suffix).is_some()
}

/// The grammar for a language, or None when this build was compiled without it.
pub fn grammar(language: &str) -> Option<Language> {
    match language {
        #[cfg(feature = "mobile")]
        "kotlin" => Some(tree_sitter_kotlin_ng::LANGUAGE.into()),
        #[cfg(feature = "mobile")]
        "java" => Some(tree_sitter_java::LANGUAGE.into()),
        #[cfg(feature = "mobile")]
        "swift" => Some(tree_sitter_swift::LANGUAGE.into()),
        #[cfg(feature = "mobile")]
        "dart" => Some(tree_sitter_dart::language()),
        #[cfg(feature = "web")]
        "typescript" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        #[cfg(feature = "web")]
        "tsx" => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
        #[cfg(feature = "web")]
        "javascript" => Some(tree_sitter_javascript::LANGUAGE.into()),
        #[cfg(feature = "backend")]
        "python" => Some(tree_sitter_python::LANGUAGE.into()),
        #[cfg(feature = "backend")]
        "go" => Some(tree_sitter_go::LANGUAGE.into()),
        #[cfg(feature = "backend")]
        "rust" => Some(tree_sitter_rust::LANGUAGE.into()),
        _ => None,
    }
}

/// Node type -> symbol kind, per language.
pub fn declarations(language: &str) -> &'static [(&'static str, &'static str)] {
    match language {
        "kotlin" => &[
            ("class_declaration", "class"),
            ("object_declaration", "object"),
            ("function_declaration", "function"),
            ("property_declaration", "property"),
        ],
        "java" => &[
            ("class_declaration", "class"),
            ("interface_declaration", "interface"),
            ("enum_declaration", "enum"),
            ("method_declaration", "function"),
        ],
        "swift" => &[
            ("class_declaration", "class"),
            ("protocol_declaration", "protocol"),
            ("function_declaration", "function"),
            ("property_declaration", "property"),
        ],
        "dart" => &[
            ("class_definition", "class"),
            ("mixin_declaration", "mixin"),
            ("enum_declaration", "enum"),
            ("function_signature", "function"),
        ],
        "typescript" | "tsx" => &[
            ("class_declaration", "class"),
            ("interface_declaration", "interface"),
            ("enum_declaration", "enum"),
            ("function_declaration", "function"),
            ("type_alias_declaration", "type"),
        ],
        "javascript" => &[
            ("class_declaration", "class"),
            ("function_declaration", "function"),
        ],
        "python" => &[
            ("class_definition", "class"),
            ("function_definition", "function"),
        ],
        "go" => &[
            ("type_declaration", "type"),
            ("function_declaration", "function"),
            ("method_declaration", "function"),
        ],
        "rust" => &[
            ("struct_item", "struct"),
            ("enum_item", "enum"),
            ("trait_item", "trait"),
            ("function_item", "function"),
            ("impl_item", "impl"),
        ],
        _ => &[],
    }
}

/// Not every grammar exposes a `name` field — Kotlin uses `simple_identifier`,
/// Dart signatures and Go methods hide the identifier one level down.
pub const IDENTIFIER_NODES: &[&str] = &[
    "identifier",
    "type_identifier",
    "simple_identifier",
    "field_identifier",
    "property_identifier",
];
