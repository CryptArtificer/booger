use super::diff::{BranchDiff, FileDiff, FileStatus};

/// Generate a concise commit message from a structural diff.
///
/// Format:
///   <summary line>
///
///   <per-file symbol changes>
pub fn draft_commit_message(diff: &BranchDiff) -> String {
    if diff.files.is_empty() {
        return "No changes to commit".to_string();
    }

    let summary = commit_summary_line(diff);
    let mut out = summary;

    let details = commit_details(diff);
    if !details.is_empty() {
        out.push_str("\n\n");
        out.push_str(&details);
    }

    out
}

fn commit_summary_line(diff: &BranchDiff) -> String {
    let s = &diff.summary;

    let mut verbs = Vec::new();
    if s.symbols_added > 0 || s.files_added > 0 {
        verbs.push("add");
    }
    if s.symbols_modified > 0 {
        verbs.push("update");
    }
    if s.symbols_removed > 0 || s.files_deleted > 0 {
        verbs.push("remove");
    }
    if verbs.is_empty() {
        verbs.push("update");
    }

    let primary_verb = verbs[0];
    let primary_verb = format!("{}{}", &primary_verb[..1].to_uppercase(), &primary_verb[1..]);

    // Collect notable symbol names (prefer added, then modified)
    let mut notable: Vec<&str> = Vec::new();
    for f in &diff.files {
        for sym in &f.added {
            if sym.kind != "import" && !sym.name.is_empty() {
                notable.push(&sym.name);
            }
        }
    }
    if notable.is_empty() {
        for f in &diff.files {
            for sym in &f.modified {
                if sym.kind != "import" && !sym.name.is_empty() {
                    notable.push(&sym.name);
                }
            }
        }
    }
    notable.truncate(3);

    if !notable.is_empty() {
        let names = notable.join(", ");
        let scope = top_level_scope(&diff.files);
        if let Some(scope) = scope {
            format!("{primary_verb} {names} in {scope}")
        } else {
            format!("{primary_verb} {names}")
        }
    } else {
        let scope = top_level_scope(&diff.files);
        let file_count = diff.files.len();
        if let Some(scope) = scope {
            format!("{primary_verb} {file_count} file(s) in {scope}")
        } else {
            format!("{primary_verb} {file_count} file(s)")
        }
    }
}

fn top_level_scope(files: &[FileDiff]) -> Option<String> {
    if files.is_empty() {
        return None;
    }
    if files.len() == 1 {
        return Some(files[0].path.clone());
    }

    // Find common path prefix
    let parts: Vec<Vec<&str>> = files
        .iter()
        .map(|f| f.path.split('/').collect::<Vec<_>>())
        .collect();

    let mut common = Vec::new();
    if let Some(first) = parts.first() {
        for (i, seg) in first.iter().enumerate() {
            if parts.iter().all(|p| p.get(i) == Some(seg)) {
                common.push(*seg);
            } else {
                break;
            }
        }
    }

    if common.is_empty() {
        None
    } else {
        Some(common.join("/"))
    }
}

fn commit_details(diff: &BranchDiff) -> String {
    let mut lines = Vec::new();

    for f in &diff.files {
        let status_prefix = match f.status {
            FileStatus::Added => "+",
            FileStatus::Modified => "~",
            FileStatus::Deleted => "-",
        };

        let has_symbols = !f.added.is_empty() || !f.modified.is_empty() || !f.removed.is_empty();
        if !has_symbols {
            continue;
        }

        lines.push(format!("[{status_prefix}] {}", f.path));

        for s in &f.added {
            lines.push(format!("  + {} {}", s.kind, s.name));
        }
        for s in &f.modified {
            lines.push(format!("  ~ {} {}", s.kind, s.name));
        }
        for s in &f.removed {
            lines.push(format!("  - {} {}", s.kind, s.name));
        }
    }

    lines.join("\n")
}

/// Generate a markdown changelog from a structural diff.
pub fn changelog(diff: &BranchDiff) -> String {
    if diff.files.is_empty() {
        return format!("No structural changes vs `{}`.\n", diff.base_ref);
    }

    let mut out = String::new();
    out.push_str(&format!("## Changes vs `{}`\n\n", diff.base_ref));
    out.push_str(&format!(
        "**{}** file(s) changed — **+{}** symbols added, **~{}** modified, **-{}** removed\n\n",
        diff.files.len(),
        diff.summary.symbols_added,
        diff.summary.symbols_modified,
        diff.summary.symbols_removed,
    ));

    // Group: new features (added symbols), changes (modified), breaking (removed)
    let all_added: Vec<_> = diff
        .files
        .iter()
        .flat_map(|f| f.added.iter().map(move |s| (f, s)))
        .filter(|(_, s)| s.kind != "import")
        .collect();

    let all_removed: Vec<_> = diff
        .files
        .iter()
        .flat_map(|f| f.removed.iter().map(move |s| (f, s)))
        .filter(|(_, s)| s.kind != "import")
        .collect();

    let all_modified: Vec<_> = diff
        .files
        .iter()
        .flat_map(|f| f.modified.iter().map(move |s| (f, s)))
        .filter(|(_, s)| s.kind != "import")
        .collect();

    let import_changes: Vec<_> = diff
        .files
        .iter()
        .flat_map(|f| {
            f.added
                .iter()
                .chain(f.modified.iter())
                .chain(f.removed.iter())
                .filter(|s| s.kind == "import")
                .map(move |s| (f, s))
        })
        .collect();

    if !all_added.is_empty() {
        out.push_str("### Added\n\n");
        for (f, s) in &all_added {
            out.push_str(&format!(
                "- `{}` {} in `{}`\n",
                s.name, s.kind, f.path
            ));
        }
        out.push('\n');
    }

    if !all_modified.is_empty() {
        out.push_str("### Modified\n\n");
        for (f, s) in &all_modified {
            out.push_str(&format!(
                "- `{}` {} in `{}`\n",
                s.name, s.kind, f.path
            ));
        }
        out.push('\n');
    }

    if !all_removed.is_empty() {
        out.push_str("### Removed\n\n");
        for (f, s) in &all_removed {
            out.push_str(&format!(
                "- `{}` {} in `{}`\n",
                s.name, s.kind, f.path
            ));
        }
        out.push('\n');
    }

    if !import_changes.is_empty() {
        out.push_str("### Dependency changes\n\n");
        for (f, s) in &import_changes {
            out.push_str(&format!("- `{}` in `{}`\n", s.name, f.path));
        }
        out.push('\n');
    }

    // New files without structural symbols
    let new_files: Vec<_> = diff
        .files
        .iter()
        .filter(|f| matches!(f.status, FileStatus::Added))
        .collect();
    if !new_files.is_empty() {
        out.push_str("### New files\n\n");
        for f in new_files {
            out.push_str(&format!("- `{}`\n", f.path));
        }
        out.push('\n');
    }

    let deleted_files: Vec<_> = diff
        .files
        .iter()
        .filter(|f| matches!(f.status, FileStatus::Deleted))
        .collect();
    if !deleted_files.is_empty() {
        out.push_str("### Deleted files\n\n");
        for f in deleted_files {
            out.push_str(&format!("- `{}`\n", f.path));
        }
        out.push('\n');
    }

    out
}
