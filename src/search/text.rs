use anyhow::{Context, Result};
use std::path::Path;

use crate::config::Config;
use crate::index;
use crate::store::sqlite::{SearchResult, Store};

pub struct SearchQuery {
    pub text: String,
    pub language: Option<String>,
    pub path_prefix: Option<String>,
    pub kind: Option<String>,
    pub max_results: usize,
    pub session_id: Option<String>,
}

impl SearchQuery {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            language: None,
            path_prefix: None,
            kind: None,
            max_results: 20,
            session_id: None,
        }
    }
}

/// Execute a full-text search against the index, applying volatile context
/// (focus boost, visited penalty) to re-rank results.
/// Automatically ensures the index is up-to-date before searching.
pub fn search(root: &Path, config: &Config, query: &SearchQuery) -> Result<Vec<SearchResult>> {
    let root = root
        .canonicalize()
        .with_context(|| format!("resolving path {}", root.display()))?;

    // Auto-index: incrementally update before searching so results are never stale.
    // This is cheap when nothing changed (walk + hash comparison only).
    let _ = index::index_directory(&root, config);

    let storage_dir = config.storage_dir(&root);
    let store = match Store::open_if_exists(&storage_dir)? {
        Some(s) => s,
        None => return Ok(Vec::new()),
    };

    // Fetch more results than requested so re-ranking has room to work
    let fetch_limit = query.max_results * 5;

    let mut results = store.search(
        &query.text,
        query.language.as_deref(),
        query.path_prefix.as_deref(),
        query.kind.as_deref(),
        fetch_limit,
    )?;

    // OR fallback: if AND-style query returned nothing and query has multiple
    // terms, retry with OR so at least partial matches surface.
    if results.is_empty() && query.text.split_whitespace().count() > 1 {
        let or_query = query.text.split_whitespace().collect::<Vec<_>>().join(" OR ");
        results = store.search(
            &or_query,
            query.language.as_deref(),
            query.path_prefix.as_deref(),
            query.kind.as_deref(),
            fetch_limit,
        )?;
    }

    if results.is_empty() {
        return Ok(results);
    }

    // ── Static re-ranking: code boost + chunk size penalty ──

    let avg_lines: f64 = {
        let total: f64 = results.iter().map(|r| (r.end_line - r.start_line + 1) as f64).sum();
        total / results.len() as f64
    };

    for result in &mut results {
        let mut boost: f64 = 0.0;

        // Code chunks (function, struct, enum, etc.) are more useful than
        // raw/doc chunks for a code search tool. Boost structural chunks.
        let is_code = !matches!(result.chunk_kind.as_str(), "raw" | "module");
        if is_code {
            boost += 3.0;
        }

        // Penalize oversized chunks. A 272-line README matches everything
        // but is rarely what you want. Scale penalty by how much larger
        // than average the chunk is (capped at 4.0).
        let lines = (result.end_line - result.start_line + 1) as f64;
        if avg_lines > 0.0 && lines > avg_lines * 2.0 {
            let oversize_ratio = lines / avg_lines;
            boost -= (oversize_ratio * 0.5).min(4.0);
        }

        result.rank -= boost;
    }

    // ── Volatile context re-ranking ──

    let focus_paths = store.get_focus_paths(query.session_id.as_deref())?;
    let visited_paths = store.get_visited_paths(query.session_id.as_deref())?;
    let annotations = store.get_annotations(None, query.session_id.as_deref())?;

    if !focus_paths.is_empty() || !visited_paths.is_empty() || !annotations.is_empty() {
        for result in &mut results {
            let mut boost: f64 = 0.0;

            for fp in &focus_paths {
                if result.file_path.starts_with(fp.as_str()) {
                    boost += 5.0;
                    break;
                }
            }

            for vp in &visited_paths {
                if result.file_path.starts_with(vp.as_str()) {
                    boost -= 3.0;
                    break;
                }
            }

            for ann in &annotations {
                if result.file_path == ann.target
                    || result
                        .chunk_name
                        .as_ref()
                        .map_or(false, |n| n == &ann.target)
                {
                    boost += 2.0;
                    break;
                }
            }

            result.rank -= boost;
        }
    }

    // Final sort and truncate
    results.sort_by(|a, b| a.rank.partial_cmp(&b.rank).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(query.max_results);
    Ok(results)
}

/// Return a short reason why search returned no results. Used by CLI and MCP
/// so agents and users see "No matches." vs "Path prefix has no indexed files." etc.
pub fn explain_empty_search(root: &Path, config: &Config, path_prefix: Option<&str>) -> String {
    let root = match root.canonicalize() {
        Ok(p) => p,
        Err(_) => root.to_path_buf(),
    };
    let storage_dir = config.storage_dir(&root);
    match Store::open_if_exists(&storage_dir) {
        Ok(Some(store)) => match store.path_has_chunks(path_prefix) {
            Ok(false) => {
                if path_prefix.is_some() {
                    "Path prefix has no indexed files.".into()
                } else {
                    "No indexed files. Run 'index' first.".into()
                }
            }
            _ => "No matches.".into(),
        },
        _ => "No index found. Run 'index' first.".into(),
    }
}
