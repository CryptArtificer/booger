use anyhow::{Context, Result};
use std::path::Path;

use crate::config::Config;
use crate::store::sqlite::{SearchResult, Store};

pub struct SearchQuery {
    pub text: String,
    pub language: Option<String>,
    pub path_prefix: Option<String>,
    pub max_results: usize,
}

impl SearchQuery {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            language: None,
            path_prefix: None,
            max_results: 20,
        }
    }
}

/// Execute a full-text search against the index.
pub fn search(root: &Path, config: &Config, query: &SearchQuery) -> Result<Vec<SearchResult>> {
    let root = root
        .canonicalize()
        .with_context(|| format!("resolving path {}", root.display()))?;
    let storage_dir = config.storage_dir(&root);
    let store = Store::open(&storage_dir)?;

    store.search(
        &query.text,
        query.language.as_deref(),
        query.path_prefix.as_deref(),
        query.max_results,
    )
}
