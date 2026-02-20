use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use crate::index::chunker;
use crate::index::walker;

#[derive(Debug, Serialize)]
pub struct BranchDiff {
    pub base_ref: String,
    pub files: Vec<FileDiff>,
    pub summary: DiffSummary,
}

#[derive(Debug, Serialize)]
pub struct FileDiff {
    pub path: String,
    pub status: FileStatus,
    pub added: Vec<SymbolChange>,
    pub removed: Vec<SymbolChange>,
    pub modified: Vec<SymbolChange>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SymbolChange {
    pub kind: String,
    pub name: String,
    pub start_line: u32,
    pub end_line: u32,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
}

#[derive(Debug, Default, Serialize)]
pub struct DiffSummary {
    pub files_added: usize,
    pub files_modified: usize,
    pub files_deleted: usize,
    pub symbols_added: usize,
    pub symbols_modified: usize,
    pub symbols_removed: usize,
}

/// Compute a structural diff between the current worktree and a base ref (branch/commit).
///
/// For each file changed on the branch, chunks both versions with tree-sitter
/// and reports which symbols were added, modified, or removed.
pub fn branch_diff(root: &Path, base_ref: &str) -> Result<BranchDiff> {
    let root = root
        .canonicalize()
        .with_context(|| format!("resolving path {}", root.display()))?;

    ensure_git_repo(&root)?;

    let changed = git_diff_files(&root, base_ref)?;
    if changed.is_empty() {
        return Ok(BranchDiff {
            base_ref: base_ref.to_string(),
            files: Vec::new(),
            summary: DiffSummary::default(),
        });
    }

    let mut files = Vec::new();
    let mut summary = DiffSummary::default();

    for (status_char, rel_path) in &changed {
        let lang = walker::detect_language(Path::new(rel_path));
        if walker::is_binary(Path::new(rel_path)) {
            continue;
        }

        let base_content = if *status_char != 'A' {
            git_show(&root, base_ref, rel_path).ok()
        } else {
            None
        };

        let head_content = if *status_char != 'D' {
            let abs = root.join(rel_path);
            std::fs::read_to_string(&abs).ok()
        } else {
            None
        };

        let base_chunks = base_content
            .as_deref()
            .map(|c| chunker::chunk_file(c, lang))
            .unwrap_or_default();
        let head_chunks = head_content
            .as_deref()
            .map(|c| chunker::chunk_file(c, lang))
            .unwrap_or_default();

        let file_status = match status_char {
            'A' => FileStatus::Added,
            'D' => FileStatus::Deleted,
            _ => FileStatus::Modified,
        };

        let (added, removed, modified) = diff_chunks(&base_chunks, &head_chunks);

        match file_status {
            FileStatus::Added => summary.files_added += 1,
            FileStatus::Modified => summary.files_modified += 1,
            FileStatus::Deleted => summary.files_deleted += 1,
        }
        summary.symbols_added += added.len();
        summary.symbols_removed += removed.len();
        summary.symbols_modified += modified.len();

        files.push(FileDiff {
            path: rel_path.clone(),
            status: file_status,
            added,
            removed,
            modified,
        });
    }

    Ok(BranchDiff {
        base_ref: base_ref.to_string(),
        files,
        summary,
    })
}

/// Compute a structural diff of staged changes vs HEAD.
/// If nothing is staged, falls back to unstaged changes (worktree vs HEAD).
pub fn staged_diff(root: &Path) -> Result<BranchDiff> {
    let root = root
        .canonicalize()
        .with_context(|| format!("resolving path {}", root.display()))?;
    ensure_git_repo(&root)?;

    let mut changed = git_staged_files(&root)?;
    let label = if changed.is_empty() {
        changed = git_unstaged_files(&root)?;
        "HEAD (unstaged)"
    } else {
        "HEAD (staged)"
    };

    if changed.is_empty() {
        return Ok(BranchDiff {
            base_ref: label.to_string(),
            files: Vec::new(),
            summary: DiffSummary::default(),
        });
    }

    let mut files = Vec::new();
    let mut summary = DiffSummary::default();

    for (status_char, rel_path) in &changed {
        let lang = walker::detect_language(Path::new(rel_path));
        if walker::is_binary(Path::new(rel_path)) {
            continue;
        }

        let base_content = if *status_char != 'A' {
            git_show(&root, "HEAD", rel_path).ok()
        } else {
            None
        };

        let head_content = if *status_char != 'D' {
            let abs = root.join(rel_path);
            std::fs::read_to_string(&abs).ok()
        } else {
            None
        };

        let base_chunks = base_content
            .as_deref()
            .map(|c| chunker::chunk_file(c, lang))
            .unwrap_or_default();
        let head_chunks = head_content
            .as_deref()
            .map(|c| chunker::chunk_file(c, lang))
            .unwrap_or_default();

        let file_status = match status_char {
            'A' => FileStatus::Added,
            'D' => FileStatus::Deleted,
            _ => FileStatus::Modified,
        };

        let (added, removed, modified) = diff_chunks(&base_chunks, &head_chunks);

        match file_status {
            FileStatus::Added => summary.files_added += 1,
            FileStatus::Modified => summary.files_modified += 1,
            FileStatus::Deleted => summary.files_deleted += 1,
        }
        summary.symbols_added += added.len();
        summary.symbols_removed += removed.len();
        summary.symbols_modified += modified.len();

        files.push(FileDiff {
            path: rel_path.clone(),
            status: file_status,
            added,
            removed,
            modified,
        });
    }

    Ok(BranchDiff {
        base_ref: label.to_string(),
        files,
        summary,
    })
}

/// Returns the list of changed files relative to the repo root.
pub fn changed_files(root: &Path, base_ref: &str) -> Result<Vec<String>> {
    let root = root
        .canonicalize()
        .with_context(|| format!("resolving path {}", root.display()))?;
    ensure_git_repo(&root)?;
    let changed = git_diff_files(&root, base_ref)?;
    Ok(changed.into_iter().map(|(_, path)| path).collect())
}

type ChunkKey = (String, String, u32); // (kind, name, occurrence index)

/// Build a map keyed by (kind, name, nth_occurrence) to handle duplicate names
/// (e.g. two `fn new` in different impl blocks within the same file).
fn build_chunk_map(chunks: &[crate::store::sqlite::ChunkInsert]) -> HashMap<ChunkKey, &crate::store::sqlite::ChunkInsert> {
    let mut counts: HashMap<(String, String), u32> = HashMap::new();
    let mut map = HashMap::new();
    for c in chunks.iter().filter(|c| c.kind != "raw") {
        let base_key = (c.kind.clone(), c.name.clone().unwrap_or_default());
        let idx = counts.entry(base_key.clone()).or_insert(0);
        map.insert((base_key.0, base_key.1, *idx), c);
        *idx += 1;
    }
    map
}

fn diff_chunks(
    base: &[crate::store::sqlite::ChunkInsert],
    head: &[crate::store::sqlite::ChunkInsert],
) -> (Vec<SymbolChange>, Vec<SymbolChange>, Vec<SymbolChange>) {
    let base_map = build_chunk_map(base);
    let head_map = build_chunk_map(head);

    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut modified = Vec::new();

    for (key, hc) in &head_map {
        let change = SymbolChange {
            kind: hc.kind.clone(),
            name: hc.name.clone().unwrap_or_else(|| key.1.clone()),
            start_line: hc.start_line,
            end_line: hc.end_line,
        };
        match base_map.get(key) {
            None => added.push(change),
            Some(bc) => {
                if bc.content != hc.content {
                    modified.push(change);
                }
            }
        }
    }

    for (key, bc) in &base_map {
        if !head_map.contains_key(key) {
            removed.push(SymbolChange {
                kind: bc.kind.clone(),
                name: bc.name.clone().unwrap_or_else(|| key.1.clone()),
                start_line: bc.start_line,
                end_line: bc.end_line,
            });
        }
    }

    (added, removed, modified)
}

fn ensure_git_repo(root: &Path) -> Result<()> {
    let output = Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .current_dir(root)
        .output()
        .context("failed to run git")?;
    if !output.status.success() {
        bail!("not a git repository: {}", root.display());
    }
    Ok(())
}

fn git_diff_files(root: &Path, base_ref: &str) -> Result<Vec<(char, String)>> {
    // Try merge-base first (three-dot diff: changes on branch only)
    let merge_base = Command::new("git")
        .args(["merge-base", base_ref, "HEAD"])
        .current_dir(root)
        .output()
        .context("git merge-base")?;

    let diff_target = if merge_base.status.success() {
        String::from_utf8_lossy(&merge_base.stdout).trim().to_string()
    } else {
        base_ref.to_string()
    };

    // -z uses NUL terminators and no quoting, which handles all path edge cases.
    // --no-renames disables rename detection so each entry is a single path.
    // Without --no-renames, R/C entries have two paths and need special parsing.
    // Renames appear as a delete + add, which is structurally accurate.
    let output = Command::new("git")
        .args(["diff", "--name-status", "-z", "--no-renames", &diff_target])
        .current_dir(root)
        .output()
        .context("git diff --name-status")?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        bail!("git diff failed: {err}");
    }

    parse_name_status_nul(&output.stdout)
}

fn git_staged_files(root: &Path) -> Result<Vec<(char, String)>> {
    let output = Command::new("git")
        .args(["diff", "--cached", "--name-status", "-z", "--no-renames"])
        .current_dir(root)
        .output()
        .context("git diff --cached")?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        bail!("git diff --cached failed: {err}");
    }
    parse_name_status_nul(&output.stdout)
}

fn git_unstaged_files(root: &Path) -> Result<Vec<(char, String)>> {
    let output = Command::new("git")
        .args(["diff", "--name-status", "-z", "--no-renames"])
        .current_dir(root)
        .output()
        .context("git diff")?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        bail!("git diff failed: {err}");
    }
    parse_name_status_nul(&output.stdout)
}

/// Parse `git diff --name-status -z` output.
/// Format: "STATUS\0PATH\0STATUS\0PATH\0..."
fn parse_name_status_nul(raw: &[u8]) -> Result<Vec<(char, String)>> {
    let text = String::from_utf8_lossy(raw);
    let parts: Vec<&str> = text.split('\0').collect();
    let mut result = Vec::new();
    let mut i = 0;
    while i < parts.len() {
        let status_part = parts[i].trim();
        if status_part.is_empty() {
            i += 1;
            continue;
        }
        let status = status_part.chars().next().unwrap_or('M');
        i += 1;
        if i < parts.len() {
            let path = parts[i].to_string();
            if !path.is_empty() {
                result.push((status, path));
            }
            i += 1;
        }
    }
    Ok(result)
}

fn git_show(root: &Path, base_ref: &str, rel_path: &str) -> Result<String> {
    let spec = format!("{base_ref}:{rel_path}");
    let output = Command::new("git")
        .args(["show", &spec])
        .current_dir(root)
        .output()
        .with_context(|| format!("git show {spec}"))?;

    if !output.status.success() {
        bail!("git show {spec} failed");
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
