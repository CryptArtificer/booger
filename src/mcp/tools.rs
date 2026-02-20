use serde_json::{json, Value};
use std::path::PathBuf;

use super::protocol::{ToolDefinition, ToolResult};
use crate::config::{Config, ProjectRegistry};
use crate::context;
use crate::index;
use crate::search::text::SearchQuery;
use crate::store::sqlite::Store;

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
                    "kind": {
                        "type": "string",
                        "description": "Filter by chunk kind: function, struct, enum, class, method, impl, import, trait, interface, type_alias, raw"
                    },
                    "output_mode": {
                        "type": "string",
                        "description": "Output mode: \"content\" shows matching lines with line numbers (default), \"files_with_matches\" shows only file locations, \"count\" shows match counts",
                        "enum": ["content", "files_with_matches", "signatures", "count"]
                    },
                    "head_limit": {
                        "type": "integer",
                        "description": "Limit number of results returned (for pagination)"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Skip first N results (for pagination)"
                    },
                    "max_lines": {
                        "type": "integer",
                        "description": "Max lines to show per result in content mode. Truncates long functions."
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
            name: "branch-diff".into(),
            description: "Structural diff between current branch and a base ref. Shows which symbols (functions, structs, imports, etc.) were added, modified, or removed. Optionally auto-focuses changed files to boost them in search.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "base": {
                        "type": "string",
                        "description": "Base branch or commit to compare against (default: main)"
                    },
                    "auto_focus": {
                        "type": "boolean",
                        "description": "If true, auto-focus changed files so subsequent searches prioritize them"
                    },
                    "session_id": {
                        "type": "string",
                        "description": "Session ID for auto-focus scope"
                    },
                    "project": project_prop()
                }
            }),
        },
        ToolDefinition {
            name: "embed".into(),
            description: "Generate embeddings for indexed chunks using ollama. Required before semantic search works. Incremental — only embeds new/changed chunks.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "model": {
                        "type": "string",
                        "description": "Ollama model name (default: nomic-embed-text)"
                    },
                    "url": {
                        "type": "string",
                        "description": "Ollama server URL (default: http://localhost:11434)"
                    },
                    "project": project_prop()
                }
            }),
        },
        ToolDefinition {
            name: "semantic-search".into(),
            description: "Semantic similarity search over embedded code chunks. Finds code by meaning, not just keywords. Requires embeddings (run 'embed' first).".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Natural language query"
                    },
                    "language": {
                        "type": "string",
                        "description": "Filter by language"
                    },
                    "path_prefix": {
                        "type": "string",
                        "description": "Filter by path prefix"
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "Maximum number of results (default: 20)"
                    },
                    "output_mode": {
                        "type": "string",
                        "description": "Output mode: \"content\" shows matching lines with line numbers (default), \"files_with_matches\" shows only file locations, \"count\" shows match counts",
                        "enum": ["content", "files_with_matches", "signatures", "count"]
                    },
                    "head_limit": {
                        "type": "integer",
                        "description": "Limit number of results returned (for pagination)"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Skip first N results (for pagination)"
                    },
                    "project": project_prop()
                },
                "required": ["query"]
            }),
        },
        ToolDefinition {
            name: "draft-commit".into(),
            description: "Generate a commit message from staged (or unstaged) changes. Analyzes structural diff (added/modified/removed symbols) to produce a meaningful message.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "project": project_prop()
                }
            }),
        },
        ToolDefinition {
            name: "changelog".into(),
            description: "Generate a markdown changelog from a structural branch diff. Shows added, modified, and removed symbols grouped by category.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "base": {
                        "type": "string",
                        "description": "Base branch or commit to compare against (default: main)"
                    },
                    "project": project_prop()
                }
            }),
        },
        ToolDefinition {
            name: "symbols".into(),
            description: "List all symbols (functions, structs, classes, imports) in a file or directory. Returns structural outline without requiring a search query.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path_prefix": {
                        "type": "string",
                        "description": "File or directory path to list symbols for (e.g. 'src/mcp/tools.rs' or 'src/mcp/')"
                    },
                    "kind": {
                        "type": "string",
                        "description": "Filter by symbol kind: function, struct, enum, class, method, impl, import, trait, interface, type_alias"
                    },
                    "output_mode": {
                        "type": "string",
                        "description": "Output mode: \"content\" shows full chunks (default), \"files_with_matches\" shows only locations, \"count\" shows counts",
                        "enum": ["content", "files_with_matches", "signatures", "count"]
                    },
                    "head_limit": {
                        "type": "integer",
                        "description": "Maximum number of results to return"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Skip first N results (for pagination)"
                    },
                    "project": project_prop()
                }
            }),
        },
        ToolDefinition {
            name: "grep".into(),
            description: "Regex/literal search within indexed chunk content. Returns matching lines with line numbers and context. Finds exact call sites, string literals, patterns — complements FTS which matches by relevance.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Regex pattern to search for (e.g. 'SecurityToken\\.verify', 'TODO|FIXME', 'fn\\s+\\w+')"
                    },
                    "path_prefix": {
                        "type": "string",
                        "description": "Filter to files under this path"
                    },
                    "kind": {
                        "type": "string",
                        "description": "Filter by chunk kind: function, struct, import, raw, etc."
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "Maximum matching lines to return (default: 50)"
                    },
                    "context_lines": {
                        "type": "integer",
                        "description": "Lines of context around each match (default: 0)"
                    },
                    "output_mode": {
                        "type": "string",
                        "description": "\"content\" shows matching lines (default), \"files_with_matches\" shows only file paths, \"count\" shows match count",
                        "enum": ["content", "files_with_matches", "count"]
                    },
                    "project": project_prop()
                },
                "required": ["pattern"]
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
        "branch-diff" => tool_branch_diff(args, project_root),
        "embed" => tool_embed(args, project_root),
        "semantic-search" => tool_semantic_search(args, project_root),
        "draft-commit" => tool_draft_commit(args, project_root),
        "changelog" => tool_changelog(args, project_root),
        "symbols" => tool_symbols(args, project_root),
        "grep" => tool_grep(args, project_root),
        "projects" => tool_projects(),
        _ => ToolResult::error(format!("Unknown tool: {name}")),
    }
}

struct FormatOpts<'a> {
    output_mode: &'a str,
    offset: usize,
    head_limit: Option<usize>,
    max_lines: Option<usize>,
}

fn parse_format_opts(args: &Value, default_mode: &str) -> (String, usize, Option<usize>, Option<usize>) {
    let output_mode = args.get("output_mode").and_then(|v| v.as_str()).unwrap_or(default_mode).to_string();
    let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let head_limit = args.get("head_limit").and_then(|v| v.as_u64()).map(|n| n as usize);
    let max_lines = args.get("max_lines").and_then(|v| v.as_u64()).map(|n| n as usize);
    (output_mode, offset, head_limit, max_lines)
}

fn format_results(results: &[crate::store::sqlite::SearchResult], opts: &FormatOpts) -> String {
    let output_mode = opts.output_mode;
    let offset = opts.offset;
    let head_limit = opts.head_limit;
    let max_lines = opts.max_lines;
    let total = results.len();
    let page: &[crate::store::sqlite::SearchResult] = if offset >= total {
        &[]
    } else {
        let end = head_limit.map_or(total, |l| (offset + l).min(total));
        &results[offset..end]
    };

    match output_mode {
        "files_with_matches" => {
            let mut out = format!("{total} result(s)");
            if offset > 0 || head_limit.is_some() {
                out.push_str(&format!(" (showing {}-{})", offset + 1, offset + page.len()));
            }
            out.push('\n');
            for r in page {
                let name = r.chunk_name.as_deref().unwrap_or("");
                out.push_str(&format!(
                    "{}:{}:{} [{}] {}\n",
                    r.file_path, r.start_line, r.end_line, r.chunk_kind, name
                ));
            }
            out
        }
        "count" => {
            format!("{total} result(s)")
        }
        "signatures" => {
            let mut out = format!("{total} result(s)");
            if offset > 0 || head_limit.is_some() {
                out.push_str(&format!(" (showing {}-{})", offset + 1, offset + page.len()));
            }
            out.push('\n');
            for r in page {
                let sig = r.content.lines().next().unwrap_or("");
                out.push_str(&format!(
                    "{}:{} [{}] {}\n",
                    r.file_path, r.start_line, r.chunk_kind, sig,
                ));
            }
            out
        }
        _ => {
            let mut out = format!("{total} result(s)");
            if offset > 0 || head_limit.is_some() {
                out.push_str(&format!(" (showing {}-{})", offset + 1, offset + page.len()));
            }
            out.push('\n');
            for (i, r) in page.iter().enumerate() {
                let name = r.chunk_name.as_deref().unwrap_or("");
                let name_display = if name.is_empty() {
                    String::new()
                } else {
                    format!(" ({name})")
                };
                out.push_str(&format!(
                    "\n── [{}] {}:{}-{} [{}{}] ──\n",
                    offset + i, r.file_path, r.start_line, r.end_line, r.chunk_kind, name_display,
                ));
                let lines: Vec<&str> = r.content.lines().collect();
                let limit = max_lines.unwrap_or(lines.len());
                let shown = limit.min(lines.len());
                for (j, line) in lines[..shown].iter().enumerate() {
                    let line_no = r.start_line as usize + j;
                    out.push_str(&format!("{line_no:>6}|{line}\n"));
                }
                if shown < lines.len() {
                    out.push_str(&format!("  ... ({} more lines)\n", lines.len() - shown));
                }
            }
            out
        }
    }
}

fn tool_search(args: &Value, project_root: &PathBuf) -> ToolResult {
    let query = match args.get("query").and_then(|v| v.as_str()) {
        Some(q) => q,
        None => return ToolResult::error("Missing required parameter: query"),
    };

    let root = match resolve_project(args, project_root) {
        Ok(r) => r,
        Err(e) => return ToolResult::error(e),
    };
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

    search_query.kind = args.get("kind").and_then(|v| v.as_str()).map(String::from);

    let (output_mode, offset, head_limit, max_lines) = parse_format_opts(args, "content");
    let opts = FormatOpts { output_mode: &output_mode, offset, head_limit, max_lines };

    match crate::search::text::search(&root, &config, &search_query) {
        Ok(results) => {
            if results.is_empty() {
                return ToolResult::success("No results found.");
            }
            ToolResult::success(format_results(&results, &opts))
        }
        Err(e) => ToolResult::error(format!("Search failed: {e}")),
    }
}

fn tool_index(args: &Value, project_root: &PathBuf) -> ToolResult {
    let root = match resolve_project(args, project_root) {
        Ok(r) => r,
        Err(e) => return ToolResult::error(e),
    };
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
    let root = match resolve_project(args, project_root) {
        Ok(r) => r,
        Err(e) => return ToolResult::error(e),
    };
    let config = Config::load(&root).unwrap_or_default();

    match index::index_status(&root, &config) {
        Ok(stats) => {
            let storage_dir = config.storage_dir(
                &root.canonicalize().unwrap_or_else(|_| root.clone()),
            );
            let kind_stats = Store::open_if_exists(&storage_dir)
                .ok()
                .flatten()
                .and_then(|s| s.kind_stats().ok())
                .unwrap_or_default();

            let mut out = format!(
                "{} files, {} chunks, {} bytes indexed ({} bytes db)\n\nLanguages:\n",
                stats.file_count, stats.chunk_count, stats.total_size_bytes, stats.db_size_bytes,
            );
            for (lang, count) in &stats.languages {
                out.push_str(&format!("  {lang}: {count}\n"));
            }
            if !kind_stats.is_empty() {
                out.push_str("\nChunk kinds:\n");
                for (kind, count) in &kind_stats {
                    out.push_str(&format!("  {kind}: {count}\n"));
                }
            }
            ToolResult::success(out)
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
    let root = match resolve_project(args, project_root) {
        Ok(r) => r,
        Err(e) => return ToolResult::error(e),
    };
    let config = Config::load(&root).unwrap_or_default();

    match context::annotations::add(&root, &config, target, note, session_id, ttl) {
        Ok(id) => ToolResult::success(format!("Annotation #{id} added to {target}")),
        Err(e) => ToolResult::error(format!("Failed to annotate: {e}")),
    }
}

fn tool_annotations(args: &Value, project_root: &PathBuf) -> ToolResult {
    let target = args.get("target").and_then(|v| v.as_str());
    let session_id = args.get("session_id").and_then(|v| v.as_str());
    let root = match resolve_project(args, project_root) {
        Ok(r) => r,
        Err(e) => return ToolResult::error(e),
    };
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
    let root = match resolve_project(args, project_root) {
        Ok(r) => r,
        Err(e) => return ToolResult::error(e),
    };
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
    let root = match resolve_project(args, project_root) {
        Ok(r) => r,
        Err(e) => return ToolResult::error(e),
    };
    let config = Config::load(&root).unwrap_or_default();

    match context::workset::visit(&root, &config, &paths, session_id) {
        Ok(()) => ToolResult::success(format!("Visited: {}", paths.join(", "))),
        Err(e) => ToolResult::error(format!("Failed to mark visited: {e}")),
    }
}

fn tool_forget(args: &Value, project_root: &PathBuf) -> ToolResult {
    let session_id = args.get("session_id").and_then(|v| v.as_str());
    let root = match resolve_project(args, project_root) {
        Ok(r) => r,
        Err(e) => return ToolResult::error(e),
    };
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

fn tool_branch_diff(args: &Value, project_root: &PathBuf) -> ToolResult {
    let root = match resolve_project(args, project_root) {
        Ok(r) => r,
        Err(e) => return ToolResult::error(e),
    };
    let detected = crate::git::diff::default_branch(&root);
    let base = args
        .get("base")
        .and_then(|v| v.as_str())
        .unwrap_or(&detected);

    match crate::git::diff::branch_diff(&root, base) {
        Ok(diff) => {
            let auto_focus = args
                .get("auto_focus")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if auto_focus && !diff.files.is_empty() {
                let config = Config::load(&root).unwrap_or_default();
                let session_id = args.get("session_id").and_then(|v| v.as_str());
                let paths: Vec<String> = diff.files.iter().map(|f| f.path.clone()).collect();
                let _ = context::workset::focus(&root, &config, &paths, session_id);
            }

            match serde_json::to_string_pretty(&diff) {
                Ok(json) => ToolResult::success(json),
                Err(e) => ToolResult::error(format!("Serialization error: {e}")),
            }
        }
        Err(e) => ToolResult::error(format!("Branch diff failed: {e}")),
    }
}

fn tool_embed(args: &Value, project_root: &PathBuf) -> ToolResult {
    let root = match resolve_project(args, project_root) {
        Ok(r) => r,
        Err(e) => return ToolResult::error(e),
    };
    let config = Config::load(&root).unwrap_or_default();
    let model = args.get("model").and_then(|v| v.as_str()).unwrap_or("nomic-embed-text");
    let url = args.get("url").and_then(|v| v.as_str()).unwrap_or("http://localhost:11434");

    let embedder = match crate::embed::ollama::OllamaEmbedder::new(url, model) {
        Ok(e) => e,
        Err(e) => return ToolResult::error(format!("Failed to connect to ollama: {e}")),
    };

    match crate::search::semantic::embed_chunks(&root, &config, &embedder) {
        Ok(stats) => {
            let summary = json!({
                "total_chunks": stats.total_chunks,
                "embedded": stats.embedded,
                "newly_embedded": stats.newly_embedded,
            });
            ToolResult::success(summary.to_string())
        }
        Err(e) => ToolResult::error(format!("Embedding failed: {e}")),
    }
}

fn tool_semantic_search(args: &Value, project_root: &PathBuf) -> ToolResult {
    let query = match args.get("query").and_then(|v| v.as_str()) {
        Some(q) => q,
        None => return ToolResult::error("Missing required parameter: query"),
    };
    let root = match resolve_project(args, project_root) {
        Ok(r) => r,
        Err(e) => return ToolResult::error(e),
    };
    let config = Config::load(&root).unwrap_or_default();

    let embedder = match crate::embed::ollama::OllamaEmbedder::default() {
        Ok(e) => e,
        Err(e) => return ToolResult::error(format!("Failed to connect to ollama: {e}")),
    };

    let mut search_query = crate::search::semantic::SemanticQuery::new(query);
    search_query.language = args.get("language").and_then(|v| v.as_str()).map(String::from);
    search_query.path_prefix = args.get("path_prefix").and_then(|v| v.as_str()).map(String::from);
    search_query.max_results = args.get("max_results").and_then(|v| v.as_u64()).map(|n| n as usize).unwrap_or(20);

    let (output_mode, offset, head_limit, max_lines) = parse_format_opts(args, "content");
    let opts = FormatOpts { output_mode: &output_mode, offset, head_limit, max_lines };

    match crate::search::semantic::search(&root, &config, &embedder, &search_query) {
        Ok(results) => {
            if results.is_empty() {
                let storage_dir = config.storage_dir(
                    &root.canonicalize().unwrap_or_else(|_| root.clone()),
                );
                let has_embeddings = Store::open_if_exists(&storage_dir)
                    .ok()
                    .flatten()
                    .and_then(|s| s.embedding_count().ok())
                    .unwrap_or(0) > 0;
                if has_embeddings {
                    return ToolResult::success("No results found.");
                }
                return ToolResult::success("No results. Run 'embed' tool first to generate embeddings.");
            }
            ToolResult::success(format_results(&results, &opts))
        }
        Err(e) => ToolResult::error(format!("Semantic search failed: {e}")),
    }
}

fn tool_draft_commit(args: &Value, project_root: &PathBuf) -> ToolResult {
    let root = match resolve_project(args, project_root) {
        Ok(r) => r,
        Err(e) => return ToolResult::error(e),
    };
    match crate::git::diff::staged_diff(&root) {
        Ok(diff) => {
            let msg = crate::git::format::draft_commit_message(&diff);
            ToolResult::success(msg)
        }
        Err(e) => ToolResult::error(format!("Draft commit failed: {e}")),
    }
}

fn tool_changelog(args: &Value, project_root: &PathBuf) -> ToolResult {
    let root = match resolve_project(args, project_root) {
        Ok(r) => r,
        Err(e) => return ToolResult::error(e),
    };
    let detected = crate::git::diff::default_branch(&root);
    let base = args
        .get("base")
        .and_then(|v| v.as_str())
        .unwrap_or(&detected);

    match crate::git::diff::branch_diff(&root, base) {
        Ok(diff) => {
            let log = crate::git::format::changelog(&diff);
            ToolResult::success(log)
        }
        Err(e) => ToolResult::error(format!("Changelog failed: {e}")),
    }
}

fn tool_symbols(args: &Value, project_root: &PathBuf) -> ToolResult {
    let root = match resolve_project(args, project_root) {
        Ok(r) => r,
        Err(e) => return ToolResult::error(e),
    };
    let config = Config::load(&root).unwrap_or_default();

    let _ = index::index_directory(&root, &config);

    let storage_dir = config.storage_dir(&root);
    let store = match Store::open_if_exists(&storage_dir) {
        Ok(Some(s)) => s,
        Ok(None) => return ToolResult::success("No index found. Run 'index' first."),
        Err(e) => return ToolResult::error(format!("Failed to open store: {e}")),
    };

    let path_prefix = args.get("path_prefix").and_then(|v| v.as_str());
    let kind = args.get("kind").and_then(|v| v.as_str());
    let (output_mode, offset, head_limit, max_lines) = parse_format_opts(args, "signatures");
    let opts = FormatOpts { output_mode: &output_mode, offset, head_limit, max_lines };

    match store.list_symbols(path_prefix, kind) {
        Ok(results) => {
            if results.is_empty() {
                return ToolResult::success("No symbols found.");
            }
            ToolResult::success(format_results(&results, &opts))
        }
        Err(e) => ToolResult::error(format!("Symbol listing failed: {e}")),
    }
}

fn tool_grep(args: &Value, project_root: &PathBuf) -> ToolResult {
    let pattern = match args.get("pattern").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return ToolResult::error("Missing required parameter: pattern"),
    };

    let regex = match regex::Regex::new(pattern) {
        Ok(r) => r,
        Err(e) => return ToolResult::error(format!("Invalid regex: {e}")),
    };

    let root = match resolve_project(args, project_root) {
        Ok(r) => r,
        Err(e) => return ToolResult::error(e),
    };
    let config = Config::load(&root).unwrap_or_default();

    let _ = index::index_directory(&root, &config);

    let storage_dir = config.storage_dir(
        &root.canonicalize().unwrap_or_else(|_| root.clone()),
    );
    let store = match Store::open_if_exists(&storage_dir) {
        Ok(Some(s)) => s,
        Ok(None) => return ToolResult::success("No index found. Run 'index' first."),
        Err(e) => return ToolResult::error(format!("Failed to open store: {e}")),
    };

    let path_prefix = args.get("path_prefix").and_then(|v| v.as_str());
    let kind = args.get("kind").and_then(|v| v.as_str());
    let max_results = args.get("max_results").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
    let context_lines = args.get("context_lines").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let output_mode = args.get("output_mode").and_then(|v| v.as_str()).unwrap_or("content");

    let chunks = match store.all_chunks(path_prefix, kind) {
        Ok(c) => c,
        Err(e) => return ToolResult::error(format!("Failed to load chunks: {e}")),
    };

    struct GrepMatch {
        file: String,
        context: Vec<(usize, String, bool)>, // (line_no, text, is_match)
    }

    let mut matches: Vec<GrepMatch> = Vec::new();
    let mut seen_files: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    'outer: for chunk in &chunks {
        let lines: Vec<&str> = chunk.content.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if regex.is_match(line) {
                seen_files.insert(chunk.file_path.clone());

                let ctx_start = i.saturating_sub(context_lines);
                let ctx_end = (i + context_lines + 1).min(lines.len());
                let context: Vec<(usize, String, bool)> = (ctx_start..ctx_end)
                    .map(|j| {
                        let ln = chunk.start_line as usize + j;
                        (ln, lines[j].to_string(), j == i)
                    })
                    .collect();

                matches.push(GrepMatch { file: chunk.file_path.clone(), context });

                if matches.len() >= max_results {
                    break 'outer;
                }
            }
        }
    }

    if matches.is_empty() {
        return ToolResult::success("No matches.");
    }

    match output_mode {
        "count" => ToolResult::success(format!(
            "at least {} match(es) in {} file(s)",
            matches.len(),
            seen_files.len()
        )),
        "files_with_matches" => {
            let mut out = format!("{} file(s)\n", seen_files.len());
            for f in &seen_files {
                out.push_str(f);
                out.push('\n');
            }
            ToolResult::success(out)
        }
        _ => {
            let mut out = format!("{} match(es)\n", matches.len());
            for m in &matches {
                for (ln, text, is_match) in &m.context {
                    let sep = if *is_match { ':' } else { '-' };
                    out.push_str(&format!("{}{sep}{ln:>6}|{text}\n", m.file));
                }
                if context_lines > 0 {
                    out.push_str("--\n");
                }
            }
            ToolResult::success(out)
        }
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
/// Errors if a project name is given but not found in the registry.
fn resolve_project(args: &Value, default_root: &PathBuf) -> Result<PathBuf, String> {
    if let Some(project_name) = args.get("project").and_then(|v| v.as_str()) {
        if let Ok(reg) = ProjectRegistry::load() {
            if let Some(path) = reg.resolve(project_name) {
                return Ok(path);
            }
        }
        return Err(format!("Unknown project: '{project_name}'. Use 'projects' tool to list registered projects."));
    }
    Ok(args.get("path")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .unwrap_or_else(|| default_root.clone()))
}
