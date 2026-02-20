use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use booger::config::Config;

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
    /// Semantic similarity search over indexed code
    Semantic {
        /// Natural language query
        query: String,
    },
    /// Annotate a file, symbol, or line range with a note
    Annotate {
        /// Target (file path, symbol, or file:line)
        target: String,
        /// The note to attach
        note: String,
    },
    /// Set focus on specific paths to boost their results
    Focus {
        /// Paths to focus on
        paths: Vec<String>,
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
    /// Start MCP server (JSON-RPC over stdio, for agent integration)
    Mcp {
        /// Project root directory
        #[arg(default_value = ".")]
        root: String,
    },
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
        Commands::Semantic { query } => {
            println!("Semantic search: {query}");
            todo!("M3: semantic search")
        }
        Commands::Mcp { root } => cmd_mcp(&root),
        Commands::Annotate { target, note } => {
            println!("Annotating {target}: {note}");
            todo!("M4: volatile context")
        }
        Commands::Focus { paths } => {
            println!("Focusing on: {}", paths.join(", "));
            todo!("M4: volatile context")
        }
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
