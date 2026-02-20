use anyhow::Result;
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

pub struct WalkConfig {
    pub max_threads: usize,
    /// Max file size in bytes to index (skip huge files)
    pub max_file_size: u64,
}

impl Default for WalkConfig {
    fn default() -> Self {
        Self {
            max_threads: 2,
            max_file_size: 1024 * 1024, // 1MB
        }
    }
}

/// Walk a directory respecting .gitignore, returning file paths.
/// Uses bounded parallelism via the `ignore` crate's parallel walker,
/// but collects results into a Vec for the caller to process in batches.
pub fn walk_files(root: &Path, config: &WalkConfig) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let walker = WalkBuilder::new(root)
        .threads(config.max_threads)
        .standard_filters(true) // .gitignore, .ignore, hidden files
        .build();

    for entry in walker {
        let entry = entry?;
        if !entry.file_type().map_or(false, |ft| ft.is_file()) {
            continue;
        }
        if let Ok(meta) = entry.metadata() {
            if meta.len() > config.max_file_size {
                continue;
            }
        }
        files.push(entry.into_path());
    }

    Ok(files)
}

const BINARY_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "ico", "webp", "svg",
    "mp3", "mp4", "wav", "avi", "mov", "mkv", "flac",
    "zip", "tar", "gz", "bz2", "xz", "7z", "rar",
    "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx",
    "exe", "dll", "so", "dylib", "o", "a", "lib",
    "wasm", "pyc", "class", "jar",
    "ttf", "otf", "woff", "woff2", "eot",
    "sqlite", "db", "db3",
    "DS_Store",
];

/// Guess whether a file is binary based on extension.
pub fn is_binary(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map_or(false, |ext| {
            BINARY_EXTENSIONS.contains(&ext.to_lowercase().as_str())
        })
}

/// Guess language from file extension. Returns None for unknown/binary.
pub fn detect_language(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?;
    match ext.to_lowercase().as_str() {
        "rs" => Some("rust"),
        "py" => Some("python"),
        "js" | "mjs" | "cjs" => Some("javascript"),
        "ts" | "mts" | "cts" => Some("typescript"),
        "tsx" => Some("tsx"),
        "jsx" => Some("jsx"),
        "go" => Some("go"),
        "c" | "h" => Some("c"),
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" => Some("cpp"),
        "java" => Some("java"),
        "rb" => Some("ruby"),
        "php" => Some("php"),
        "swift" => Some("swift"),
        "kt" | "kts" => Some("kotlin"),
        "scala" => Some("scala"),
        "zig" => Some("zig"),
        "lua" => Some("lua"),
        "sh" | "bash" | "zsh" => Some("shell"),
        "sql" => Some("sql"),
        "html" | "htm" => Some("html"),
        "css" => Some("css"),
        "scss" | "sass" => Some("scss"),
        "json" => Some("json"),
        "yaml" | "yml" => Some("yaml"),
        "toml" => Some("toml"),
        "xml" => Some("xml"),
        "md" | "markdown" => Some("markdown"),
        "txt" => Some("text"),
        "proto" => Some("protobuf"),
        "graphql" | "gql" => Some("graphql"),
        "dockerfile" => Some("dockerfile"),
        "makefile" => Some("makefile"),
        "cmake" => Some("cmake"),
        "nix" => Some("nix"),
        "tf" | "hcl" => Some("hcl"),
        "el" | "lisp" | "cl" => Some("lisp"),
        "clj" | "cljs" | "cljc" => Some("clojure"),
        "ex" | "exs" => Some("elixir"),
        "erl" | "hrl" => Some("erlang"),
        "hs" => Some("haskell"),
        "ml" | "mli" => Some("ocaml"),
        "r" => Some("r"),
        "dart" => Some("dart"),
        "vue" => Some("vue"),
        "svelte" => Some("svelte"),
        _ => None,
    }
}
