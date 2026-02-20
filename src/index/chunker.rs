use crate::store::sqlite::ChunkInsert;
use tree_sitter::{Language, Node, Parser};

/// Get a tree-sitter Language for the given language name.
fn get_language(lang: &str) -> Option<Language> {
    match lang {
        "rust" => Some(tree_sitter_rust::LANGUAGE.into()),
        "python" => Some(tree_sitter_python::LANGUAGE.into()),
        "javascript" | "jsx" => Some(tree_sitter_javascript::LANGUAGE.into()),
        "typescript" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        "tsx" => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
        "go" => Some(tree_sitter_go::LANGUAGE.into()),
        "c" => Some(tree_sitter_c::LANGUAGE.into()),
        _ => None,
    }
}

/// Chunk a file into structural units using tree-sitter.
/// Falls back to a single raw chunk for unsupported languages.
pub fn chunk_file(content: &str, language: Option<&str>) -> Vec<ChunkInsert> {
    if content.is_empty() {
        return Vec::new();
    }

    if let Some(lang) = language {
        if let Some(ts_lang) = get_language(lang) {
            if let Some(chunks) = chunk_with_treesitter(content, ts_lang, lang) {
                if !chunks.is_empty() {
                    return chunks;
                }
            }
        }
    }

    raw_chunk(content)
}

fn chunk_with_treesitter(content: &str, language: Language, lang: &str) -> Option<Vec<ChunkInsert>> {
    let mut parser = Parser::new();
    parser.set_language(&language).ok()?;
    let tree = parser.parse(content, None)?;
    let root = tree.root_node();

    let mut chunks = Vec::new();
    collect_chunks(root, content.as_bytes(), lang, &mut chunks);
    Some(chunks)
}

fn collect_chunks(node: Node, source: &[u8], lang: &str, chunks: &mut Vec<ChunkInsert>) {
    let source_str = std::str::from_utf8(source).unwrap_or("");

    if let Some((kind, name)) = classify_node(&node, source_str, lang) {
        let start = node.start_position();
        let end = node.end_position();
        let text = &source_str[node.byte_range()];

        chunks.push(ChunkInsert {
            kind,
            name,
            content: text.to_string(),
            start_line: (start.row + 1) as u32,
            end_line: (end.row + 1) as u32,
            start_byte: node.start_byte() as u32,
            end_byte: node.end_byte() as u32,
        });
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_chunks(child, source, lang, chunks);
    }
}

/// Classify a tree-sitter node into a chunk kind + optional name.
/// Returns None for nodes we don't want as standalone chunks.
fn classify_node<'a>(node: &Node, source: &'a str, lang: &str) -> Option<(String, Option<String>)> {
    let kind = node.kind();

    match lang {
        "rust" => classify_rust(node, kind, source),
        "python" => classify_python(node, kind, source),
        "javascript" | "jsx" | "typescript" | "tsx" => classify_js_ts(node, kind, source),
        "go" => classify_go(node, kind, source),
        "c" => classify_c(node, kind, source),
        _ => None,
    }
}

fn find_child_by_field<'a>(node: &Node<'a>, field: &str) -> Option<Node<'a>> {
    node.child_by_field_name(field)
}

fn node_text<'a>(node: &Node, source: &'a str) -> &'a str {
    &source[node.byte_range()]
}

fn classify_rust(node: &Node, kind: &str, source: &str) -> Option<(String, Option<String>)> {
    match kind {
        "function_item" => {
            let name = find_child_by_field(node, "name").map(|n| node_text(&n, source).to_string());
            Some(("function".into(), name))
        }
        "struct_item" => {
            let name = find_child_by_field(node, "name").map(|n| node_text(&n, source).to_string());
            Some(("struct".into(), name))
        }
        "enum_item" => {
            let name = find_child_by_field(node, "name").map(|n| node_text(&n, source).to_string());
            Some(("enum".into(), name))
        }
        "impl_item" => {
            let name = find_child_by_field(node, "type").map(|n| node_text(&n, source).to_string());
            Some(("impl".into(), name))
        }
        "trait_item" => {
            let name = find_child_by_field(node, "name").map(|n| node_text(&n, source).to_string());
            Some(("trait".into(), name))
        }
        "mod_item" => {
            let name = find_child_by_field(node, "name").map(|n| node_text(&n, source).to_string());
            Some(("module".into(), name))
        }
        "type_item" => {
            let name = find_child_by_field(node, "name").map(|n| node_text(&n, source).to_string());
            Some(("type_alias".into(), name))
        }
        "const_item" | "static_item" => {
            let name = find_child_by_field(node, "name").map(|n| node_text(&n, source).to_string());
            Some(("constant".into(), name))
        }
        "macro_definition" => {
            let name = find_child_by_field(node, "name").map(|n| node_text(&n, source).to_string());
            Some(("macro".into(), name))
        }
        _ => None,
    }
}

fn classify_python(node: &Node, kind: &str, source: &str) -> Option<(String, Option<String>)> {
    match kind {
        "function_definition" => {
            let name = find_child_by_field(node, "name").map(|n| node_text(&n, source).to_string());
            Some(("function".into(), name))
        }
        "class_definition" => {
            let name = find_child_by_field(node, "name").map(|n| node_text(&n, source).to_string());
            Some(("class".into(), name))
        }
        "decorated_definition" => {
            // Recurse into the actual definition inside the decorator
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if let Some(result) = classify_python(&child, child.kind(), source) {
                    return Some(result);
                }
            }
            None
        }
        _ => None,
    }
}

fn classify_js_ts(node: &Node, kind: &str, source: &str) -> Option<(String, Option<String>)> {
    match kind {
        "function_declaration" => {
            let name = find_child_by_field(node, "name").map(|n| node_text(&n, source).to_string());
            Some(("function".into(), name))
        }
        "class_declaration" => {
            let name = find_child_by_field(node, "name").map(|n| node_text(&n, source).to_string());
            Some(("class".into(), name))
        }
        "method_definition" => {
            let name = find_child_by_field(node, "name").map(|n| node_text(&n, source).to_string());
            Some(("method".into(), name))
        }
        "lexical_declaration" | "variable_declaration" => {
            // Catch `const foo = () => {}` and `export const`
            let text = node_text(node, source);
            if text.contains("=>") || text.contains("function") {
                let name = node
                    .child(1) // declarator
                    .and_then(|d| d.child_by_field_name("name"))
                    .map(|n| node_text(&n, source).to_string());
                Some(("function".into(), name))
            } else {
                None
            }
        }
        "export_statement" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if let Some(result) = classify_js_ts(&child, child.kind(), source) {
                    return Some(result);
                }
            }
            None
        }
        "interface_declaration" => {
            let name = find_child_by_field(node, "name").map(|n| node_text(&n, source).to_string());
            Some(("interface".into(), name))
        }
        "type_alias_declaration" => {
            let name = find_child_by_field(node, "name").map(|n| node_text(&n, source).to_string());
            Some(("type_alias".into(), name))
        }
        "enum_declaration" => {
            let name = find_child_by_field(node, "name").map(|n| node_text(&n, source).to_string());
            Some(("enum".into(), name))
        }
        _ => None,
    }
}

fn classify_go(node: &Node, kind: &str, source: &str) -> Option<(String, Option<String>)> {
    match kind {
        "function_declaration" => {
            let name = find_child_by_field(node, "name").map(|n| node_text(&n, source).to_string());
            Some(("function".into(), name))
        }
        "method_declaration" => {
            let name = find_child_by_field(node, "name").map(|n| node_text(&n, source).to_string());
            Some(("method".into(), name))
        }
        "type_declaration" => {
            let name = node.child(1).map(|n| node_text(&n, source).to_string());
            Some(("type".into(), name))
        }
        _ => None,
    }
}

fn classify_c(node: &Node, kind: &str, source: &str) -> Option<(String, Option<String>)> {
    match kind {
        "function_definition" => {
            let name = find_child_by_field(node, "declarator")
                .and_then(|d| d.child_by_field_name("declarator"))
                .map(|n| node_text(&n, source).to_string());
            Some(("function".into(), name))
        }
        "struct_specifier" => {
            let name = find_child_by_field(node, "name").map(|n| node_text(&n, source).to_string());
            Some(("struct".into(), name))
        }
        "enum_specifier" => {
            let name = find_child_by_field(node, "name").map(|n| node_text(&n, source).to_string());
            Some(("enum".into(), name))
        }
        "type_definition" => {
            Some(("type_alias".into(), None))
        }
        _ => None,
    }
}

fn raw_chunk(content: &str) -> Vec<ChunkInsert> {
    let line_count = content.lines().count() as u32;
    let byte_len = content.len() as u32;

    vec![ChunkInsert {
        kind: "raw".to_string(),
        name: None,
        content: content.to_string(),
        start_line: 1,
        end_line: line_count.max(1),
        start_byte: 0,
        end_byte: byte_len,
    }]
}
