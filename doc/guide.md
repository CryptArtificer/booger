# User Guide

## Configuration

Run `booger init` to create a `.booger/config.toml`:

```toml
[storage]
max_size_bytes = 0            # 0 = unlimited

[resources]
max_threads = 6               # 0 = half available cores
max_memory_bytes = 268435456  # 256 MB
batch_size = 500

[embed]
type = "none"                 # "ollama" or "openai"
```

## Multi-Project Registry

Register projects by name for easy cross-project access:

```bash
booger project add myapp /path/to/myapp
booger project add lib /path/to/lib
booger project list

# Register and index all git repos under a parent directory
booger project add-all /path/to/parent
```

Projects are stored in `~/.booger/projects.json` and can be referenced
by name in any tool call via the `project` parameter.

## Supported Languages

[Tree-sitter](https://tree-sitter.github.io/tree-sitter/) structural
chunking extracts functions, structs, classes, methods, enums, traits,
interfaces, type aliases, constants, macros, and import/use statements:

| Language | Grammar | Imports |
|---|---|---|
| Rust | [tree-sitter-rust](https://github.com/tree-sitter/tree-sitter-rust) | `use` declarations |
| Python | [tree-sitter-python](https://github.com/tree-sitter/tree-sitter-python) | `import` / `from ... import` |
| JavaScript | [tree-sitter-javascript](https://github.com/tree-sitter/tree-sitter-javascript) | `import` / `require()` |
| TypeScript / TSX | [tree-sitter-typescript](https://github.com/tree-sitter/tree-sitter-typescript) | `import` / `require()` |
| Go | [tree-sitter-go](https://github.com/tree-sitter/tree-sitter-go) | `import` declarations |
| C | [tree-sitter-c](https://github.com/tree-sitter/tree-sitter-c) | `#include` |

All other file types are indexed as whole-file chunks and are still
searchable via FTS5.

## Volatile Context (Working Memory)

Beyond static indexing, booger maintains a volatile context layer
that turns it from a search engine into a working memory:

- **Annotations**: attach notes to files, symbols, or line ranges.
  Supports session scoping and TTL for auto-expiry. Notes appear
  inline as `[note]` lines in search results.
- **Focus**: mark paths as focused to boost their search ranking (+5).
- **Visited**: mark paths as visited to deprioritize them (-3).
- **Forget**: clear all context, or just a specific session's context.

```bash
booger annotate src/parser.rs "Has a known bug in error recovery"
booger focus src/mcp src/search
booger visit src/config.rs
booger forget                    # clears ALL context
booger forget --session abc      # clears only session 'abc'
```

See the [volatile context diagram](architecture.md#volatile-context-layer).

## Git Integration

Booger uses Tree-sitter to structurally diff branches — not lines,
but symbols:

```bash
# What changed on this branch vs main?
booger branch-diff main
# Output:
# [~] src/store/sqlite.rs
#     + function open_if_exists (99:108)
#     ~ function search (206:270)
#     - function old_helper (50:55)

# Auto-generated commit message from staged/unstaged changes
booger draft-commit

# Markdown changelog for a PR description
booger changelog main

# Auto-focus changed files so search prioritizes them
booger branch-diff main --focus
```

The default branch is auto-detected from `origin/HEAD` or local
`main`/`master` refs. See the
[git integration diagram](architecture.md#git-integration-flow).

## Semantic Search (Optional)

Booger can generate embeddings via a local [Ollama](https://ollama.ai/)
instance for meaning-based search:

```bash
# Install and start ollama (one-time setup)
brew install ollama
ollama serve &
ollama pull nomic-embed-text

# Generate embeddings for a project
booger embed /path/to/project

# Search by meaning
booger semantic "error handling in database layer"
```

Uses [nomic-embed-text](https://ollama.com/library/nomic-embed-text)
(274 MB, 768 dimensions). Embeddings are stored as f32 BLOBs in SQLite
and searched via cosine similarity.
