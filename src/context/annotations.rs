use anyhow::{Context, Result};
use std::path::Path;

use crate::config::Config;
use crate::store::sqlite::{Annotation, Store};

pub fn add(
    root: &Path,
    config: &Config,
    target: &str,
    note: &str,
    session_id: Option<&str>,
    ttl_seconds: Option<i64>,
) -> Result<i64> {
    let store = open_store_rw(root, config)?;
    store.add_annotation(target, note, session_id, ttl_seconds)
}

pub fn list(
    root: &Path,
    config: &Config,
    target: Option<&str>,
    session_id: Option<&str>,
) -> Result<Vec<Annotation>> {
    let store = match open_store_ro(root, config)? {
        Some(s) => s,
        None => return Ok(Vec::new()),
    };
    store.clear_expired_annotations()?;
    store.get_annotations(target, session_id)
}

pub fn remove(root: &Path, config: &Config, id: i64) -> Result<()> {
    match open_store_ro(root, config)? {
        Some(store) => store.delete_annotation(id),
        None => Ok(()),
    }
}

pub fn clear_session(root: &Path, config: &Config, session_id: &str) -> Result<usize> {
    match open_store_ro(root, config)? {
        Some(store) => store.clear_session_annotations(session_id),
        None => Ok(0),
    }
}

fn open_store_rw(root: &Path, config: &Config) -> Result<Store> {
    let root = root
        .canonicalize()
        .with_context(|| format!("resolving path {}", root.display()))?;
    let storage_dir = config.storage_dir(&root);
    Store::open(&storage_dir)
}

fn open_store_ro(root: &Path, config: &Config) -> Result<Option<Store>> {
    let root = root
        .canonicalize()
        .with_context(|| format!("resolving path {}", root.display()))?;
    let storage_dir = config.storage_dir(&root);
    Store::open_if_exists(&storage_dir)
}
