use anyhow::{Context, Result};
use std::path::Path;

use crate::config::Config;
use crate::store::sqlite::{Store, WorksetEntry};

pub fn focus(
    root: &Path,
    config: &Config,
    paths: &[String],
    session_id: Option<&str>,
) -> Result<()> {
    let store = open_store(root, config)?;
    for p in paths {
        store.add_to_workset(p, "focus", session_id)?;
    }
    Ok(())
}

pub fn visit(
    root: &Path,
    config: &Config,
    paths: &[String],
    session_id: Option<&str>,
) -> Result<()> {
    let store = open_store(root, config)?;
    for p in paths {
        store.add_to_workset(p, "visited", session_id)?;
    }
    Ok(())
}

pub fn unfocus(root: &Path, config: &Config, paths: &[String]) -> Result<()> {
    let store = open_store(root, config)?;
    for p in paths {
        store.remove_from_workset(p, "focus")?;
    }
    Ok(())
}

pub fn list(
    root: &Path,
    config: &Config,
    kind: Option<&str>,
    session_id: Option<&str>,
) -> Result<Vec<WorksetEntry>> {
    let store = open_store(root, config)?;
    store.get_workset(kind, session_id)
}

pub fn clear(root: &Path, config: &Config, session_id: Option<&str>) -> Result<usize> {
    let store = open_store(root, config)?;
    store.clear_workset(session_id)
}

fn open_store(root: &Path, config: &Config) -> Result<Store> {
    let root = root
        .canonicalize()
        .with_context(|| format!("resolving path {}", root.display()))?;
    let storage_dir = config.storage_dir(&root);
    Store::open(&storage_dir)
}
