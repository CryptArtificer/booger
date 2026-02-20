use serde_json::{json, Value};
use std::path::PathBuf;

use super::protocol::{ToolDefinition, ToolResult};
use crate::config::Config;
use crate::index;
use crate::search::text::SearchQuery;

pub fn list_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "search".into(),
            description: "Full-text search over indexed code. Returns matching code chunks with file path, line numbers, and content.".into(),
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
                        "description": "Maximum number of results to return (default: 20)"
                    }
                },
                "required": ["query"]
            }),
        },
        ToolDefinition {
            name: "index".into(),
            description: "Index a directory for searching. Performs incremental updates — only re-indexes changed files.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to directory to index (default: project root)"
                    }
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
                    }
                }
            }),
        },
    ]
}

pub fn call_tool(name: &str, args: &Value, project_root: &PathBuf) -> ToolResult {
    match name {
        "search" => tool_search(args, project_root),
        "index" => tool_index(args, project_root),
        "status" => tool_status(args, project_root),
        _ => ToolResult::error(format!("Unknown tool: {name}")),
    }
}

fn tool_search(args: &Value, project_root: &PathBuf) -> ToolResult {
    let query = match args.get("query").and_then(|v| v.as_str()) {
        Some(q) => q,
        None => return ToolResult::error("Missing required parameter: query"),
    };

    let root = resolve_path(args.get("path"), project_root);
    let config = Config::load(&root).unwrap_or_default();

    let mut search_query = SearchQuery::new(query);
    search_query.language = args.get("language").and_then(|v| v.as_str()).map(String::from);
    search_query.path_prefix = args.get("path_prefix").and_then(|v| v.as_str()).map(String::from);
    search_query.max_results = args
        .get("max_results")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or(20);

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
    let root = resolve_path(args.get("path"), project_root);
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
    let root = resolve_path(args.get("path"), project_root);
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

fn resolve_path(path_arg: Option<&Value>, project_root: &PathBuf) -> PathBuf {
    path_arg
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .unwrap_or_else(|| project_root.clone())
}
