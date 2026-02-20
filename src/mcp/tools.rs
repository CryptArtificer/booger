use serde_json::{json, Value};
use std::path::PathBuf;

use super::protocol::{ToolDefinition, ToolResult};
use crate::config::{Config, ProjectRegistry};
use crate::context;
use crate::index;
use crate::search::text::SearchQuery;

fn project_prop() -> Value {
    json!({ "type": "string", "description": "Registered project name or path (use 'projects' tool to list)" })
}

pub fn list_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "search".into(),
            description: "Full-text search over indexed code. Returns matching code chunks ranked by relevance, boosted by focus paths and penalized for visited paths.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query (FTS5 syntax: terms, phrases in quotes, OR, NOT)"
                    },
                    "language": {
                        "type": "string",
                        "description": "Filter by language (e.g. rust, python, typescript, go, c)"
                    },
                    "path_prefix": {
                        "type": "string",
                        "description": "Filter results to files under this path prefix"
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "Maximum number of results (default: 20)"
                    },
                    "session_id": {
                        "type": "string",
                        "description": "Session ID for volatile context (focus/visited) awareness"
                    },
                    "project": {
                        "type": "string",
                        "description": "Registered project name or path (use 'projects' tool to list)"
                    }
                },
                "required": ["query"]
            }),
        },
        ToolDefinition {
            name: "index".into(),
            description: "Index a directory for searching. Incremental — only re-indexes changed files.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to directory to index (default: project root)"
                    },
                    "project": project_prop()
                }
            }),
        },
        ToolDefinition {
            name: "status".into(),
            description: "Show index statistics: file count, chunk count, languages, sizes.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to indexed directory (default: project root)"
                    },
                    "project": project_prop()
                }
            }),
        },
        ToolDefinition {
            name: "annotate".into(),
            description: "Attach a note to a file, symbol, or line range. Notes are included in context and can influence search. Supports TTL for auto-expiry.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "target": {
                        "type": "string",
                        "description": "What to annotate: file path, symbol name, or 'file:line'"
                    },
                    "note": {
                        "type": "string",
                        "description": "The note to attach"
                    },
                    "session_id": {
                        "type": "string",
                        "description": "Session ID to scope the annotation"
                    },
                    "ttl_seconds": {
                        "type": "integer",
                        "description": "Time-to-live in seconds (omit for no expiry)"
                    },
                    "project": project_prop()
                },
                "required": ["target", "note"]
            }),
        },
        ToolDefinition {
            name: "annotations".into(),
            description: "List all annotations, optionally filtered by target or session.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "target": {
                        "type": "string",
                        "description": "Filter by target"
                    },
                    "session_id": {
                        "type": "string",
                        "description": "Filter by session ID"
                    },
                    "project": project_prop()
                }
            }),
        },
        ToolDefinition {
            name: "focus".into(),
            description: "Mark paths as focused — search results from these paths are boosted.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "paths": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Paths to focus on"
                    },
                    "session_id": {
                        "type": "string",
                        "description": "Session ID"
                    },
                    "project": project_prop()
                },
                "required": ["paths"]
            }),
        },
        ToolDefinition {
            name: "visit".into(),
            description: "Mark paths as visited — search results from these paths are deprioritized.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "paths": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Paths to mark as visited"
                    },
                    "session_id": {
                        "type": "string",
                        "description": "Session ID"
                    },
                    "project": project_prop()
                },
                "required": ["paths"]
            }),
        },
        ToolDefinition {
            name: "forget".into(),
            description: "Clear volatile context: annotations and working set. Optionally scoped to a session.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "Session to clear (omit to clear all)"
                    },
                    "project": project_prop()
                }
            }),
        },
        ToolDefinition {
            name: "projects".into(),
            description: "List all registered projects. Use project names in the 'project' parameter of other tools to target a specific project.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
    ]
}

pub fn call_tool(name: &str, args: &Value, project_root: &PathBuf) -> ToolResult {
    match name {
        "search" => tool_search(args, project_root),
        "index" => tool_index(args, project_root),
        "status" => tool_status(args, project_root),
        "annotate" => tool_annotate(args, project_root),
        "annotations" => tool_annotations(args, project_root),
        "focus" => tool_focus(args, project_root),
        "visit" => tool_visit(args, project_root),
        "forget" => tool_forget(args, project_root),
        "projects" => tool_projects(),
        _ => ToolResult::error(format!("Unknown tool: {name}")),
    }
}

fn tool_search(args: &Value, project_root: &PathBuf) -> ToolResult {
    let query = match args.get("query").and_then(|v| v.as_str()) {
        Some(q) => q,
        None => return ToolResult::error("Missing required parameter: query"),
    };

    let root = resolve_project(args, project_root);
    let config = Config::load(&root).unwrap_or_default();

    let mut search_query = SearchQuery::new(query);
    search_query.language = args.get("language").and_then(|v| v.as_str()).map(String::from);
    search_query.path_prefix = args.get("path_prefix").and_then(|v| v.as_str()).map(String::from);
    search_query.max_results = args
        .get("max_results")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or(20);
    search_query.session_id = args.get("session_id").and_then(|v| v.as_str()).map(String::from);

    match crate::search::text::search(&root, &config, &search_query) {
        Ok(results) => {
            if results.is_empty() {
                return ToolResult::success("No results found.");
            }
            match serde_json::to_string_pretty(&results) {
                Ok(json) => ToolResult::success(json),
                Err(e) => ToolResult::error(format!("Serialization error: {e}")),
            }
        }
        Err(e) => ToolResult::error(format!("Search failed: {e}")),
    }
}

fn tool_index(args: &Value, project_root: &PathBuf) -> ToolResult {
    let root = resolve_project(args, project_root);
    let config = Config::load(&root).unwrap_or_default();

    match index::index_directory(&root, &config) {
        Ok(result) => {
            let summary = json!({
                "files_scanned": result.files_scanned,
                "files_indexed": result.files_indexed,
                "files_unchanged": result.files_unchanged,
                "files_skipped": result.files_skipped,
                "chunks_created": result.chunks_created,
            });
            ToolResult::success(summary.to_string())
        }
        Err(e) => ToolResult::error(format!("Indexing failed: {e}")),
    }
}

fn tool_status(args: &Value, project_root: &PathBuf) -> ToolResult {
    let root = resolve_project(args, project_root);
    let config = Config::load(&root).unwrap_or_default();

    match index::index_status(&root, &config) {
        Ok(stats) => {
            let summary = json!({
                "file_count": stats.file_count,
                "chunk_count": stats.chunk_count,
                "total_size_bytes": stats.total_size_bytes,
                "db_size_bytes": stats.db_size_bytes,
                "languages": stats.languages.iter()
                    .map(|(lang, count)| json!({"language": lang, "files": count}))
                    .collect::<Vec<_>>(),
            });
            ToolResult::success(serde_json::to_string_pretty(&summary).unwrap_or_default())
        }
        Err(e) => ToolResult::error(format!("Status failed: {e}")),
    }
}

fn tool_annotate(args: &Value, project_root: &PathBuf) -> ToolResult {
    let target = match args.get("target").and_then(|v| v.as_str()) {
        Some(t) => t,
        None => return ToolResult::error("Missing required parameter: target"),
    };
    let note = match args.get("note").and_then(|v| v.as_str()) {
        Some(n) => n,
        None => return ToolResult::error("Missing required parameter: note"),
    };
    let session_id = args.get("session_id").and_then(|v| v.as_str());
    let ttl = args.get("ttl_seconds").and_then(|v| v.as_i64());
    let root = resolve_project(args, project_root);
    let config = Config::load(&root).unwrap_or_default();

    match context::annotations::add(&root, &config, target, note, session_id, ttl) {
        Ok(id) => ToolResult::success(format!("Annotation #{id} added to {target}")),
        Err(e) => ToolResult::error(format!("Failed to annotate: {e}")),
    }
}

fn tool_annotations(args: &Value, project_root: &PathBuf) -> ToolResult {
    let target = args.get("target").and_then(|v| v.as_str());
    let session_id = args.get("session_id").and_then(|v| v.as_str());
    let root = resolve_project(args, project_root);
    let config = Config::load(&root).unwrap_or_default();

    match context::annotations::list(&root, &config, target, session_id) {
        Ok(anns) => match serde_json::to_string_pretty(&anns) {
            Ok(json) => ToolResult::success(json),
            Err(e) => ToolResult::error(format!("Serialization error: {e}")),
        },
        Err(e) => ToolResult::error(format!("Failed to list annotations: {e}")),
    }
}

fn tool_focus(args: &Value, project_root: &PathBuf) -> ToolResult {
    let paths = match args.get("paths").and_then(|v| v.as_array()) {
        Some(arr) => arr.iter().filter_map(|v| v.as_str().map(String::from)).collect::<Vec<_>>(),
        None => return ToolResult::error("Missing required parameter: paths"),
    };
    let session_id = args.get("session_id").and_then(|v| v.as_str());
    let root = resolve_project(args, project_root);
    let config = Config::load(&root).unwrap_or_default();

    match context::workset::focus(&root, &config, &paths, session_id) {
        Ok(()) => ToolResult::success(format!("Focused: {}", paths.join(", "))),
        Err(e) => ToolResult::error(format!("Failed to set focus: {e}")),
    }
}

fn tool_visit(args: &Value, project_root: &PathBuf) -> ToolResult {
    let paths = match args.get("paths").and_then(|v| v.as_array()) {
        Some(arr) => arr.iter().filter_map(|v| v.as_str().map(String::from)).collect::<Vec<_>>(),
        None => return ToolResult::error("Missing required parameter: paths"),
    };
    let session_id = args.get("session_id").and_then(|v| v.as_str());
    let root = resolve_project(args, project_root);
    let config = Config::load(&root).unwrap_or_default();

    match context::workset::visit(&root, &config, &paths, session_id) {
        Ok(()) => ToolResult::success(format!("Visited: {}", paths.join(", "))),
        Err(e) => ToolResult::error(format!("Failed to mark visited: {e}")),
    }
}

fn tool_forget(args: &Value, project_root: &PathBuf) -> ToolResult {
    let session_id = args.get("session_id").and_then(|v| v.as_str());
    let root = resolve_project(args, project_root);
    let config = Config::load(&root).unwrap_or_default();

    let anns = context::annotations::clear_session(
        &root,
        &config,
        session_id.unwrap_or(""),
    );
    let ws = context::workset::clear(&root, &config, session_id);

    match (anns, ws) {
        (Ok(a), Ok(w)) => ToolResult::success(format!("Cleared {a} annotations, {w} workset entries")),
        (Err(e), _) | (_, Err(e)) => ToolResult::error(format!("Failed to clear: {e}")),
    }
}

fn tool_projects() -> ToolResult {
    match ProjectRegistry::load() {
        Ok(reg) => {
            if reg.projects.is_empty() {
                return ToolResult::success("No registered projects. Use the CLI: booger project add <name> <path>");
            }
            let list: Vec<_> = reg
                .projects
                .iter()
                .map(|(name, entry)| {
                    json!({ "name": name, "path": entry.path.to_string_lossy() })
                })
                .collect();
            match serde_json::to_string_pretty(&list) {
                Ok(json) => ToolResult::success(json),
                Err(e) => ToolResult::error(format!("Serialization error: {e}")),
            }
        }
        Err(e) => ToolResult::error(format!("Failed to load registry: {e}")),
    }
}

/// Resolve the project root from tool arguments.
/// Priority: 'project' (registry lookup) > 'path' (literal) > default root.
fn resolve_project(args: &Value, default_root: &PathBuf) -> PathBuf {
    if let Some(project_name) = args.get("project").and_then(|v| v.as_str()) {
        if let Ok(reg) = ProjectRegistry::load() {
            if let Some(path) = reg.resolve(project_name) {
                return path;
            }
        }
    }
    args.get("path")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .unwrap_or_else(|| default_root.clone())
}
