use anyhow::{Context, Result};
use std::path::Path;

use crate::config::Config;
use crate::index;
use crate::store::sqlite::{SearchResult, Store};

pub struct SearchQuery {
    pub text: String,
    pub language: Option<String>,
    pub path_prefix: Option<String>,
    pub max_results: usize,
    pub session_id: Option<String>,
}

impl SearchQuery {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            language: None,
            path_prefix: None,
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
    let store = Store::open(&storage_dir)?;

    // Fetch more results than requested so re-ranking has room to work
    let fetch_limit = query.max_results * 3;

    let mut results = store.search(
        &query.text,
        query.language.as_deref(),
        query.path_prefix.as_deref(),
        fetch_limit,
    )?;

    // Apply volatile context re-ranking
    let focus_paths = store.get_focus_paths(query.session_id.as_deref())?;
    let visited_paths = store.get_visited_paths(query.session_id.as_deref())?;
    let annotations = store.get_annotations(None, query.session_id.as_deref())?;

    if !focus_paths.is_empty() || !visited_paths.is_empty() || !annotations.is_empty() {
        for result in &mut results {
            let mut boost: f64 = 0.0;

            // Focus: boost results whose file path starts with a focused path
            for fp in &focus_paths {
                if result.file_path.starts_with(fp.as_str()) {
                    boost += 5.0;
                    break;
                }
            }

            // Visited: penalize results from already-seen files
            for vp in &visited_paths {
                if result.file_path.starts_with(vp.as_str()) {
                    boost -= 3.0;
                    break;
                }
            }

            // Annotations: boost results that have annotations on their target
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

            // FTS5 rank is negative (closer to 0 = better), so we subtract boost
            result.rank -= boost;
        }

        results.sort_by(|a, b| a.rank.partial_cmp(&b.rank).unwrap_or(std::cmp::Ordering::Equal));
    }

    results.truncate(query.max_results);
    Ok(results)
}
