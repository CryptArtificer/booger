pub mod chunker;
pub mod hasher;
pub mod walker;

use anyhow::{Context, Result};
use std::path::Path;

use crate::config::Config;
use crate::store::sqlite::Store;
use walker::{WalkConfig, detect_language, is_binary, walk_files};

pub struct IndexResult {
    pub files_scanned: usize,
    pub files_indexed: usize,
    pub files_skipped: usize,
    pub files_unchanged: usize,
    pub chunks_created: usize,
}

/// Run a full indexing pass on a directory.
pub fn index_directory(root: &Path, config: &Config) -> Result<IndexResult> {
    let root = root
        .canonicalize()
        .with_context(|| format!("resolving path {}", root.display()))?;

    let storage_dir = config.storage_dir(&root);
    let store = Store::open(&storage_dir)?;

    let walk_config = WalkConfig {
        max_threads: config.effective_threads(),
        ..Default::default()
    };

    let files = walk_files(&root, &walk_config)?;
    let total = files.len();

    let mut result = IndexResult {
        files_scanned: total,
        files_indexed: 0,
        files_skipped: 0,
        files_unchanged: 0,
        chunks_created: 0,
    };

    store.begin_transaction()?;
    let mut batch_count = 0;

    for path in &files {
        if is_binary(path) {
            result.files_skipped += 1;
            continue;
        }

        let rel_path = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        let content_hash = match hasher::hash_file(path) {
            Ok(h) => h,
            Err(_) => {
                result.files_skipped += 1;
                continue;
            }
        };

        if let Ok(Some(existing)) = store.get_file(&rel_path) {
            if existing.content_hash == content_hash {
                result.files_unchanged += 1;
                continue;
            }
            store.delete_chunks_for_file(existing.id)?;
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => {
                result.files_skipped += 1;
                continue;
            }
        };

        let language = detect_language(path);
        let size_bytes = content.len() as i64;

        let file_id = store.upsert_file(&rel_path, &content_hash, size_bytes, language)?;
        let chunks = chunker::chunk_file(&content, language);
        result.chunks_created += chunks.len();
        store.insert_chunks(file_id, &chunks)?;
        result.files_indexed += 1;

        batch_count += 1;
        if batch_count >= config.resources.batch_size {
            store.commit_transaction()?;
            store.begin_transaction()?;
            batch_count = 0;
        }
    }

    store.commit_transaction()?;

    Ok(result)
}

/// Get index statistics for a directory. Returns empty stats if no index exists.
pub fn index_status(root: &Path, config: &Config) -> Result<crate::store::sqlite::IndexStats> {
    let root = root
        .canonicalize()
        .with_context(|| format!("resolving path {}", root.display()))?;
    let storage_dir = config.storage_dir(&root);
    match Store::open_if_exists(&storage_dir)? {
        Some(store) => store.stats(&storage_dir),
        None => Ok(crate::store::sqlite::IndexStats::empty()),
    }
}
