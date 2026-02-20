use anyhow::{Context, Result};
use std::path::Path;

use crate::config::Config;
use crate::embed::{cosine_similarity, Embedder};
use crate::store::sqlite::{SearchResult, Store};

pub struct SemanticQuery {
    pub text: String,
    pub max_results: usize,
    pub path_prefix: Option<String>,
    pub language: Option<String>,
}

impl SemanticQuery {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            max_results: 20,
            path_prefix: None,
            language: None,
        }
    }
}

pub struct EmbedStats {
    pub total_chunks: i64,
    pub embedded: i64,
    pub newly_embedded: usize,
}

/// Generate embeddings for all chunks that don't have one yet.
/// Returns stats about what was processed.
pub fn embed_chunks(root: &Path, config: &Config, embedder: &dyn Embedder) -> Result<EmbedStats> {
    let root = root
        .canonicalize()
        .with_context(|| format!("resolving path {}", root.display()))?;

    // Auto-index first
    let _ = crate::index::index_directory(&root, config);

    let storage_dir = config.storage_dir(&root);
    let store = Store::open(&storage_dir)?;

    let model = embedder.model_name();
    let pending = store.chunks_needing_embedding(model)?;

    let total_chunks: i64 = store.chunk_count()?;
    let already_embedded = store.embedding_count()?;

    if pending.is_empty() {
        return Ok(EmbedStats {
            total_chunks,
            embedded: already_embedded,
            newly_embedded: 0,
        });
    }

    let batch_size = 32;
    let mut newly_embedded = 0;

    for batch in pending.chunks(batch_size) {
        let mut batch_entries: Vec<(i64, &str, Vec<f32>)> = Vec::new();

        for (id, content) in batch {
            match embedder.embed(content) {
                Ok(emb) => batch_entries.push((*id, model, emb)),
                Err(e) => {
                    eprintln!("  warning: skipping chunk {id} ({}): {e}", &content[..content.len().min(50)]);
                    continue;
                }
            }
        }

        let entries: Vec<(i64, &str, &[f32])> = batch_entries
            .iter()
            .map(|(id, m, emb)| (*id, *m, emb.as_slice()))
            .collect();

        store.upsert_embeddings_batch(&entries)?;
        newly_embedded += entries.len();
    }

    Ok(EmbedStats {
        total_chunks,
        embedded: already_embedded + newly_embedded as i64,
        newly_embedded,
    })
}

/// Semantic search: embed the query, then find nearest chunks by cosine similarity.
pub fn search(
    root: &Path,
    config: &Config,
    embedder: &dyn Embedder,
    query: &SemanticQuery,
) -> Result<Vec<SearchResult>> {
    let root = root
        .canonicalize()
        .with_context(|| format!("resolving path {}", root.display()))?;

    let storage_dir = config.storage_dir(&root);
    let store = match Store::open_if_exists(&storage_dir)? {
        Some(s) => s,
        None => return Ok(Vec::new()),
    };

    let all_embeddings = store.all_embeddings()?;
    if all_embeddings.is_empty() {
        return Ok(Vec::new());
    }

    let query_embedding = embedder.embed(&query.text)?;

    // Score all chunks by cosine similarity
    let mut scored: Vec<(i64, f32)> = all_embeddings
        .iter()
        .map(|(id, emb)| (*id, cosine_similarity(&query_embedding, emb)))
        .collect();

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(query.max_results * 3);

    // Resolve chunk IDs to full results, applying filters
    let mut results = Vec::new();
    for (chunk_id, similarity) in scored {
        if let Some(mut result) = store.chunk_by_id(chunk_id)? {
            if let Some(ref lang) = query.language {
                if result.language.as_deref() != Some(lang.as_str()) {
                    continue;
                }
            }
            if let Some(ref prefix) = query.path_prefix {
                if !result.file_path.starts_with(prefix.as_str()) {
                    continue;
                }
            }
            result.rank = -(similarity as f64);
            results.push(result);
            if results.len() >= query.max_results {
                break;
            }
        }
    }

    Ok(results)
}
