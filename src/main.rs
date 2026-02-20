use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use booger::config::Config;
use booger::embed::Embedder;

#[derive(Parser)]
#[command(name = "booger", version, about = "I found it! — Local code search for AI agents")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Index a directory for searching
    Index {
        /// Path to the directory to index
        #[arg(default_value = ".")]
        path: String,
    },
    /// Full-text search over indexed code
    Search {
        /// Search query
        query: String,
        /// Filter by language (e.g. rust, python, typescript)
        #[arg(short, long)]
        language: Option<String>,
        /// Filter by path prefix (e.g. src/index)
        #[arg(short, long)]
        path: Option<String>,
        /// Project root to search in
        #[arg(short, long, default_value = ".")]
        root: String,
        /// Max number of results
        #[arg(short = 'n', long, default_value = "20")]
        max_results: usize,
        /// Output as JSON (for agent consumption)
        #[arg(long)]
        json: bool,
    },
    /// Semantic similarity search over indexed code (requires embeddings)
    Semantic {
        /// Natural language query
        query: String,
        /// Project root
        #[arg(short, long, default_value = ".")]
        root: String,
        /// Filter by language
        #[arg(short, long)]
        language: Option<String>,
        /// Filter by path prefix
        #[arg(short, long)]
        path: Option<String>,
        /// Max results
        #[arg(short = 'n', long, default_value = "20")]
        max_results: usize,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Generate embeddings for indexed chunks (requires ollama)
    Embed {
        /// Project root
        #[arg(default_value = ".")]
        path: String,
        /// Ollama model name
        #[arg(long, default_value = "nomic-embed-text")]
        model: String,
        /// Ollama server URL
        #[arg(long, default_value = "http://localhost:11434")]
        url: String,
    },
    /// Annotate a file, symbol, or line range with a note
    Annotate {
        /// Target (file path, symbol, or file:line)
        target: String,
        /// The note to attach
        note: String,
        /// Project root
        #[arg(short, long, default_value = ".")]
        root: String,
        /// Session ID (scopes annotation to a session)
        #[arg(short, long)]
        session: Option<String>,
        /// TTL in seconds (annotation auto-expires)
        #[arg(short, long)]
        ttl: Option<i64>,
    },
    /// List annotations
    Annotations {
        /// Filter by target
        #[arg(short, long)]
        target: Option<String>,
        /// Project root
        #[arg(short, long, default_value = ".")]
        root: String,
        /// Session ID filter
        #[arg(short, long)]
        session: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Set focus on specific paths to boost their search results
    Focus {
        /// Paths to focus on
        paths: Vec<String>,
        /// Project root
        #[arg(short, long, default_value = ".")]
        root: String,
        /// Session ID
        #[arg(short, long)]
        session: Option<String>,
    },
    /// Mark paths as visited (deprioritize in search)
    Visit {
        /// Paths to mark as visited
        paths: Vec<String>,
        /// Project root
        #[arg(short, long, default_value = ".")]
        root: String,
        /// Session ID
        #[arg(short, long)]
        session: Option<String>,
    },
    /// Clear volatile context (annotations, working set)
    Forget {
        /// Project root
        #[arg(short, long, default_value = ".")]
        root: String,
        /// Session ID to clear (omit to clear all)
        #[arg(short, long)]
        session: Option<String>,
    },
    /// Show index status and statistics
    Status {
        /// Path to the indexed directory
        #[arg(default_value = ".")]
        path: String,
    },
    /// Initialize a .booger config in a directory
    Init {
        /// Path to the directory
        #[arg(default_value = ".")]
        path: String,
    },
    /// Show structural diff between current branch and a base ref
    BranchDiff {
        /// Base branch or commit to compare against (auto-detects if omitted)
        base: Option<String>,
        /// Project root
        #[arg(short, long, default_value = ".")]
        root: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Auto-focus changed files in volatile context
        #[arg(long)]
        focus: bool,
        /// Session ID for auto-focus
        #[arg(short, long)]
        session: Option<String>,
    },
    /// Draft a commit message from staged (or unstaged) changes
    DraftCommit {
        /// Project root
        #[arg(short, long, default_value = ".")]
        root: String,
    },
    /// Generate a structural changelog between current state and a base ref
    Changelog {
        /// Base branch or commit to compare against (auto-detects if omitted)
        base: Option<String>,
        /// Project root
        #[arg(short, long, default_value = ".")]
        root: String,
    },
    /// Start MCP server (JSON-RPC over stdio, for agent integration)
    Mcp {
        /// Default project root directory
        #[arg(default_value = ".")]
        root: String,
    },
    /// Manage registered projects
    #[command(subcommand)]
    Project(ProjectCommands),
}

#[derive(Subcommand)]
enum ProjectCommands {
    /// Register a project by name
    Add {
        /// Short name for the project
        name: String,
        /// Path to the project directory
        #[arg(default_value = ".")]
        path: String,
    },
    /// Remove a registered project
    Remove {
        /// Project name to remove
        name: String,
    },
    /// List all registered projects
    List,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Index { path } => cmd_index(&path),
        Commands::Status { path } => cmd_status(&path),
        Commands::Init { path } => cmd_init(&path),
        Commands::Search { query, language, path, root, max_results, json } => {
            cmd_search(&root, &query, language.as_deref(), path.as_deref(), max_results, json)
        }
        Commands::Semantic { query, root, language, path, max_results, json } => {
            cmd_semantic(&root, &query, language.as_deref(), path.as_deref(), max_results, json)
        }
        Commands::Embed { path, model, url } => {
            cmd_embed(&path, &model, &url)
        }
        Commands::BranchDiff { base, root, json, focus, session } => {
            let base = base.unwrap_or_else(|| booger::git::diff::default_branch(std::path::Path::new(&root)));
            cmd_branch_diff(&root, &base, json, focus, session.as_deref())
        }
        Commands::DraftCommit { root } => cmd_draft_commit(&root),
        Commands::Changelog { base, root } => {
            let base = base.unwrap_or_else(|| booger::git::diff::default_branch(std::path::Path::new(&root)));
            cmd_changelog(&root, &base)
        }
        Commands::Mcp { root } => cmd_mcp(&root),
        Commands::Annotate { target, note, root, session, ttl } => {
            cmd_annotate(&root, &target, &note, session.as_deref(), ttl)
        }
        Commands::Annotations { target, root, session, json } => {
            cmd_annotations(&root, target.as_deref(), session.as_deref(), json)
        }
        Commands::Focus { paths, root, session } => {
            cmd_focus(&root, &paths, session.as_deref())
        }
        Commands::Visit { paths, root, session } => {
            cmd_visit(&root, &paths, session.as_deref())
        }
        Commands::Forget { root, session } => {
            cmd_forget(&root, session.as_deref())
        }
        Commands::Project(sub) => cmd_project(sub),
    }
}

fn cmd_index(path: &str) -> Result<()> {
    let root = PathBuf::from(path);
    let config = Config::load(&root).unwrap_or_default();

    eprintln!(
        "Indexing {} (threads: {}, batch: {})",
        root.display(),
        config.effective_threads(),
        config.resources.batch_size,
    );

    let result = booger::index::index_directory(&root, &config)?;

    eprintln!(
        "Done. scanned={} indexed={} unchanged={} skipped={} chunks={}",
        result.files_scanned,
        result.files_indexed,
        result.files_unchanged,
        result.files_skipped,
        result.chunks_created,
    );

    Ok(())
}

fn cmd_status(path: &str) -> Result<()> {
    let root = PathBuf::from(path);
    let config = Config::load(&root).unwrap_or_default();
    let stats = booger::index::index_status(&root, &config)?;

    println!("Index status for {}", root.canonicalize()?.display());
    println!("  Files:       {}", stats.file_count);
    println!("  Chunks:      {}", stats.chunk_count);
    println!("  Source size:  {}", format_bytes(stats.total_size_bytes));
    println!("  Index size:   {}", format_bytes(stats.db_size_bytes));
    if !stats.languages.is_empty() {
        println!("  Languages:");
        for (lang, count) in &stats.languages {
            println!("    {lang}: {count} files");
        }
    }

    Ok(())
}

fn cmd_search(
    root: &str,
    query: &str,
    language: Option<&str>,
    path_prefix: Option<&str>,
    max_results: usize,
    json: bool,
) -> Result<()> {
    let root = PathBuf::from(root);
    let config = Config::load(&root).unwrap_or_default();

    let mut search_query = booger::search::text::SearchQuery::new(query);
    search_query.language = language.map(String::from);
    search_query.path_prefix = path_prefix.map(String::from);
    search_query.max_results = max_results;

    let results = booger::search::text::search(&root, &config, &search_query)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&results)?);
    } else {
        if results.is_empty() {
            eprintln!("No results found.");
            return Ok(());
        }
        eprintln!("{} result(s)\n", results.len());
        for (i, r) in results.iter().enumerate() {
            let name = r.chunk_name.as_deref().unwrap_or("");
            let name_display = if name.is_empty() {
                String::new()
            } else {
                format!(" ({name})")
            };
            println!(
                "── [{i}] {}:{}-{} [{}{}] ──",
                r.file_path, r.start_line, r.end_line, r.chunk_kind, name_display,
            );
            // Show a truncated preview: first 10 lines
            let preview: String = r.content.lines().take(10).collect::<Vec<_>>().join("\n");
            println!("{preview}");
            let total_lines = r.content.lines().count();
            if total_lines > 10 {
                println!("  ... ({} more lines)", total_lines - 10);
            }
            println!();
        }
    }

    Ok(())
}

fn cmd_annotate(
    root: &str,
    target: &str,
    note: &str,
    session_id: Option<&str>,
    ttl: Option<i64>,
) -> Result<()> {
    let root = PathBuf::from(root);
    let config = Config::load(&root).unwrap_or_default();
    let id = booger::context::annotations::add(&root, &config, target, note, session_id, ttl)?;
    eprintln!("Annotation #{id} added to {target}");
    Ok(())
}

fn cmd_annotations(
    root: &str,
    target: Option<&str>,
    session_id: Option<&str>,
    json: bool,
) -> Result<()> {
    let root = PathBuf::from(root);
    let config = Config::load(&root).unwrap_or_default();
    let anns = booger::context::annotations::list(&root, &config, target, session_id)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&anns)?);
    } else if anns.is_empty() {
        eprintln!("No annotations.");
    } else {
        for a in &anns {
            let expires = a
                .expires_at
                .as_deref()
                .map(|e| format!(" (expires: {e})"))
                .unwrap_or_default();
            println!("  #{} [{}]{} — {}", a.id, a.target, expires, a.note);
        }
    }
    Ok(())
}

fn cmd_focus(root: &str, paths: &[String], session_id: Option<&str>) -> Result<()> {
    let root = PathBuf::from(root);
    let config = Config::load(&root).unwrap_or_default();
    booger::context::workset::focus(&root, &config, paths, session_id)?;
    eprintln!("Focused: {}", paths.join(", "));
    Ok(())
}

fn cmd_visit(root: &str, paths: &[String], session_id: Option<&str>) -> Result<()> {
    let root = PathBuf::from(root);
    let config = Config::load(&root).unwrap_or_default();
    booger::context::workset::visit(&root, &config, paths, session_id)?;
    eprintln!("Visited: {}", paths.join(", "));
    Ok(())
}

fn cmd_forget(root: &str, session_id: Option<&str>) -> Result<()> {
    let root = PathBuf::from(root);
    let config = Config::load(&root).unwrap_or_default();
    let anns = booger::context::annotations::clear_session(
        &root,
        &config,
        session_id.unwrap_or(""),
    )?;
    let ws = booger::context::workset::clear(&root, &config, session_id)?;
    eprintln!("Cleared {anns} annotations, {ws} workset entries");
    Ok(())
}

fn cmd_branch_diff(
    root: &str,
    base_ref: &str,
    json: bool,
    auto_focus: bool,
    session_id: Option<&str>,
) -> Result<()> {
    let root = PathBuf::from(root);
    let diff = booger::git::diff::branch_diff(&root, base_ref)?;

    if auto_focus && !diff.files.is_empty() {
        let config = Config::load(&root).unwrap_or_default();
        let paths: Vec<String> = diff.files.iter().map(|f| f.path.clone()).collect();
        booger::context::workset::focus(&root, &config, &paths, session_id)?;
        eprintln!("Auto-focused {} changed files", paths.len());
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&diff)?);
    } else {
        eprintln!(
            "Branch diff vs {} — {} file(s) ({} added, {} modified, {} deleted)",
            diff.base_ref,
            diff.files.len(),
            diff.summary.files_added,
            diff.summary.files_modified,
            diff.summary.files_deleted,
        );
        eprintln!(
            "Symbols: +{} added, ~{} modified, -{} removed\n",
            diff.summary.symbols_added,
            diff.summary.symbols_modified,
            diff.summary.symbols_removed,
        );

        for f in &diff.files {
            let status = match f.status {
                booger::git::diff::FileStatus::Added => "+",
                booger::git::diff::FileStatus::Modified => "~",
                booger::git::diff::FileStatus::Deleted => "-",
            };
            println!("[{status}] {}", f.path);

            for s in &f.added {
                println!("    + {} {} ({}:{})", s.kind, s.name, s.start_line, s.end_line);
            }
            for s in &f.modified {
                println!("    ~ {} {} ({}:{})", s.kind, s.name, s.start_line, s.end_line);
            }
            for s in &f.removed {
                println!("    - {} {} ({}:{})", s.kind, s.name, s.start_line, s.end_line);
            }
        }
    }

    Ok(())
}

fn cmd_embed(path: &str, model: &str, url: &str) -> Result<()> {
    let root = PathBuf::from(path);
    let config = Config::load(&root).unwrap_or_default();

    eprintln!("Connecting to ollama at {url} (model: {model})...");
    let embedder = booger::embed::ollama::OllamaEmbedder::new(url, model)?;
    eprintln!("Model loaded ({} dimensions)", embedder.dimensions());

    let stats = booger::search::semantic::embed_chunks(&root, &config, &embedder)?;
    eprintln!(
        "Done. {}/{} chunks embedded ({} new)",
        stats.embedded, stats.total_chunks, stats.newly_embedded,
    );
    Ok(())
}

fn cmd_semantic(
    root: &str,
    query: &str,
    language: Option<&str>,
    path_prefix: Option<&str>,
    max_results: usize,
    json: bool,
) -> Result<()> {
    let root = PathBuf::from(root);
    let config = Config::load(&root).unwrap_or_default();

    let embedder = booger::embed::ollama::OllamaEmbedder::default()?;

    let mut search_query = booger::search::semantic::SemanticQuery::new(query);
    search_query.language = language.map(String::from);
    search_query.path_prefix = path_prefix.map(String::from);
    search_query.max_results = max_results;

    let results = booger::search::semantic::search(&root, &config, &embedder, &search_query)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&results)?);
    } else {
        if results.is_empty() {
            eprintln!("No results. Run `booger embed` first to generate embeddings.");
            return Ok(());
        }
        eprintln!("{} result(s)\n", results.len());
        for (i, r) in results.iter().enumerate() {
            let name = r.chunk_name.as_deref().unwrap_or("");
            let name_display = if name.is_empty() {
                String::new()
            } else {
                format!(" ({name})")
            };
            let similarity = -r.rank;
            println!(
                "── [{i}] {}:{}-{} [{}{}] (sim: {similarity:.3}) ──",
                r.file_path, r.start_line, r.end_line, r.chunk_kind, name_display,
            );
            let preview: String = r.content.lines().take(10).collect::<Vec<_>>().join("\n");
            println!("{preview}");
            let total_lines = r.content.lines().count();
            if total_lines > 10 {
                println!("  ... ({} more lines)", total_lines - 10);
            }
            println!();
        }
    }

    Ok(())
}

fn cmd_draft_commit(root: &str) -> Result<()> {
    let root = PathBuf::from(root);
    let diff = booger::git::diff::staged_diff(&root)?;
    let msg = booger::git::format::draft_commit_message(&diff);
    println!("{msg}");
    Ok(())
}

fn cmd_changelog(root: &str, base_ref: &str) -> Result<()> {
    let root = PathBuf::from(root);
    let diff = booger::git::diff::branch_diff(&root, base_ref)?;
    let log = booger::git::format::changelog(&diff);
    println!("{log}");
    Ok(())
}

fn cmd_project(sub: ProjectCommands) -> Result<()> {
    use booger::config::ProjectRegistry;
    match sub {
        ProjectCommands::Add { name, path } => {
            let abs_path = PathBuf::from(&path)
                .canonicalize()
                .unwrap_or_else(|_| PathBuf::from(&path));
            let mut reg = ProjectRegistry::load()?;
            reg.add(name.clone(), abs_path.clone());
            reg.save()?;
            eprintln!("Registered project '{name}' -> {}", abs_path.display());
        }
        ProjectCommands::Remove { name } => {
            let mut reg = ProjectRegistry::load()?;
            if reg.remove(&name) {
                reg.save()?;
                eprintln!("Removed project '{name}'");
            } else {
                eprintln!("Project '{name}' not found");
            }
        }
        ProjectCommands::List => {
            let reg = ProjectRegistry::load()?;
            if reg.projects.is_empty() {
                eprintln!("No registered projects.");
            } else {
                for (name, entry) in &reg.projects {
                    println!("  {name}: {}", entry.path.display());
                }
            }
        }
    }
    Ok(())
}

fn cmd_mcp(root: &str) -> Result<()> {
    let project_root = PathBuf::from(root)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(root));
    booger::mcp::server::run(project_root)
}

fn cmd_init(path: &str) -> Result<()> {
    let root = PathBuf::from(path)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(path));
    let config = Config::default();
    config.save(&root)?;
    eprintln!("Initialized .booger in {}", root.display());
    Ok(())
}

fn format_bytes(bytes: impl Into<i128>) -> String {
    let bytes = bytes.into().unsigned_abs();
    const KB: u128 = 1024;
    const MB: u128 = 1024 * KB;
    const GB: u128 = 1024 * MB;
    match bytes {
        b if b >= GB => format!("{:.1} GB", b as f64 / GB as f64),
        b if b >= MB => format!("{:.1} MB", b as f64 / MB as f64),
        b if b >= KB => format!("{:.1} KB", b as f64 / KB as f64),
        b => format!("{b} B"),
    }
}
