use clap::{Parser, Subcommand};

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
        path: String,
    },
    /// Full-text search over indexed code
    Search {
        /// Search query
        query: String,
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
    Status,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Index { path } => {
            println!("Indexing: {path}");
            todo!("M1: indexing")
        }
        Commands::Search { query } => {
            println!("Searching: {query}");
            todo!("M2: text search")
        }
        Commands::Semantic { query } => {
            println!("Semantic search: {query}");
            todo!("M3: semantic search")
        }
        Commands::Annotate { target, note } => {
            println!("Annotating {target}: {note}");
            todo!("M4: volatile context")
        }
        Commands::Focus { paths } => {
            println!("Focusing on: {}", paths.join(", "));
            todo!("M4: volatile context")
        }
        Commands::Status => {
            println!("booger status");
            todo!("M1: status reporting")
        }
    }
}
