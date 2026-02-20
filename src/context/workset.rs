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
    let store = open_store_rw(root, config)?;
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
    let store = open_store_rw(root, config)?;
    for p in paths {
        store.add_to_workset(p, "visited", session_id)?;
    }
    Ok(())
}

pub fn unfocus(root: &Path, config: &Config, paths: &[String]) -> Result<()> {
    match open_store_ro(root, config)? {
        Some(store) => {
            for p in paths {
                store.remove_from_workset(p, "focus")?;
            }
            Ok(())
        }
        None => Ok(()),
    }
}

pub fn list(
    root: &Path,
    config: &Config,
    kind: Option<&str>,
    session_id: Option<&str>,
) -> Result<Vec<WorksetEntry>> {
    match open_store_ro(root, config)? {
        Some(store) => store.get_workset(kind, session_id),
        None => Ok(Vec::new()),
    }
}

pub fn clear(root: &Path, config: &Config, session_id: Option<&str>) -> Result<usize> {
    match open_store_ro(root, config)? {
        Some(store) => store.clear_workset(session_id),
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
