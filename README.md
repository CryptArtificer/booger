# Booger

<p align="center">
  <img src="doc/rw-booger.png" width="400" alt="Ralph Wiggum: Hey, I found a booger!" />
</p>

<p align="center">
  <strong>A local code search engine, index, and working memory for AI agents.</strong>
</p>

<p align="center">
  One binary &middot; Zero dependencies &middot; Zero background processes &middot; 14 MB
</p>

---

Booger indexes your codebases using [tree-sitter](https://tree-sitter.github.io/tree-sitter/)
for structural chunking, stores everything in [SQLite](https://sqlite.org/) with
[FTS5](https://www.sqlite.org/fts5.html), and exposes it all via
[MCP](https://modelcontextprotocol.io/) or CLI. It's designed to be the tool that
AI agents use to efficiently find and reason about code.

18 tools. 7 languages. Structural search, references, git-aware diffs, semantic
embeddings, volatile working memory — all in a single static binary.

**[Architecture Diagrams](doc/architecture.md)** &middot;
**[Concepts & Technology](doc/concepts.md)** &middot;
**[Project Plan](PLAN.md)**

## Install

### From source (recommended)

Requires [Rust 1.75+](https://rustup.rs/):

```bash
git clone https://github.com/CryptArtificer/booger.git
cd booger
make install
```

Or without make:

```bash
cargo install --path .
```

### Verify

```bash
booger --version
booger status          # shows index stats (creates nothing if no index exists)
```

## Quick Start

```bash
# Index a project (incremental — only processes changed files)
booger index /path/to/project

# Search (auto-indexes if needed, you can skip the step above)
booger search "parse config"

# Structural outline of a file
booger symbols src/main.rs

# Find all usages of a function
booger references dispatch

# What changed on this branch?
booger branch-diff main

# Generate a commit message from staged changes
booger draft-commit
```

## MCP Server

Booger runs as an [MCP](https://modelcontextprotocol.io/) tool server for
AI agent integration. MCP is a JSON-RPC 2.0 protocol over stdio that lets
agents discover and call tools programmatically.

```bash
booger mcp /path/to/project
```

### Cursor

Add to `.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "booger": {
      "command": "booger",
      "args": ["mcp", "/path/to/default/project"]
    }
  }
}
```

### Codex

Add to `codex` CLI configuration or use the `--mcp` flag:

```bash
codex --mcp "booger:booger mcp /path/to/project"
```

### Development (hot-reload)

For active development on booger itself, use the proxy script so
`cargo install` takes effect immediately without restarting the
MCP session:

```json
{
  "mcpServers": {
    "booger": {
      "command": "/path/to/booger/booger-proxy.sh",
      "args": ["/path/to/default/project"]
    }
  }
}
```

The proxy spawns a fresh `booger` process per JSON-RPC request,
so rebuilding picks up changes instantly. New tool definitions
still require a session restart for the client to discover them.

### Available MCP Tools

#### Search & Discovery

| Tool | Description |
|---|---|
| `search` | Full-text search with [BM25](https://en.wikipedia.org/wiki/Okapi_BM25) ranking, volatile context re-ranking, kind/language/path filters |
| `semantic-search` | Similarity search via local embeddings (requires [Ollama](https://ollama.ai/)) |
| `hybrid-search` | Combined FTS + semantic search with tunable alpha weighting |
| `grep` | Regex/literal search within indexed chunks — returns matching lines with context |
| `references` | Find all usages of a symbol: definitions, call sites, type references, imports |
| `symbols` | List all symbols in a file/directory — structural outline with smart signatures |

#### Indexing & Embeddings

| Tool | Description |
|---|---|
| `index` | Index a directory (incremental, BLAKE3 hash-based change detection) |
| `status` | Index stats: files, chunks, languages, chunk kind breakdown |
| `embed` | Generate embeddings for semantic search via [Ollama](https://ollama.ai/) |

#### Volatile Context (Working Memory)

| Tool | Description |
|---|---|
| `annotate` | Attach notes to files/symbols/line-ranges with optional TTL |
| `annotations` | List annotations (filterable by target and session) |
| `focus` | Boost search results for specific paths |
| `visit` | Deprioritize already-seen paths in search |
| `forget` | Clear volatile context (all or session-scoped) |

#### Git Integration

| Tool | Description |
|---|---|
| `branch-diff` | Structural diff between branches — added/modified/removed symbols per file |
| `draft-commit` | Generate commit message from staged/unstaged structural changes |
| `changelog` | Generate markdown changelog from branch diff |

#### Registry

| Tool | Description |
|---|---|
| `projects` | List registered projects |

All tools accept an optional `project` parameter — a registered project
name or a literal path. Unknown project names return an error (no silent
fallback).

### Output Modes

Search tools support multiple output modes to minimize token usage:

| Mode | Use case | Example |
|---|---|---|
| `content` | Full code with line numbers (default) | Function body with `[note]` annotations |
| `signatures` | One-line smart signatures | `fn search(&self, query: &str, ...) -> Result<Vec<SearchResult>>` |
| `files_with_matches` | Just file paths and line ranges | `src/store/sqlite.rs:209:280 [function] search` |
| `count` | Just the number | `42 result(s)` |

Additional controls: `head_limit` / `offset` for pagination, `max_lines`
to cap content output, `kind` to filter by chunk type.

## How Search Works

```
Query
  → auto-index (walk + BLAKE3 hash, skip unchanged files)
  → FTS5 full-text search (Porter stemmer + unicode61, BM25 ranking)
  → static re-ranking:
      code chunks boosted over docs/raw (+3)
      oversized chunks penalized (up to -4)
  → volatile context re-ranking:
      focused paths boosted (+5)
      visited paths penalized (-3)
      annotated targets boosted (+2)
  → inline annotations injected into results as [note] lines
  → return top N results
```

Search results are individual functions, structs, and classes —
not entire files. Container blocks (`impl`, `class`, `trait`) are split
into their child methods so you get precisely the code you need.

The `references` tool goes further: given a symbol name, it finds every
chunk that mentions it and classifies each hit as `[definition]`,
`[call]`, `[type]`, `[import]`, or `[reference]`, along with which
function the usage lives in.

`hybrid-search` runs both FTS and semantic search, normalizes scores
to [0,1], and merges with configurable alpha (default 0.7 FTS / 0.3
semantic). Degrades gracefully when embeddings aren't available.

Read-only operations (`status`, `search`, `annotations`, `forget`) never
create a `.booger/` directory as a side effect.

See the [search pipeline diagram](doc/architecture.md#search-pipeline)
for a visual breakdown.

## Smart Signatures

When booger indexes code, it extracts clean type signatures using
tree-sitter — everything up to the function body, without the `{`:

```
pub fn search(&self, query: &str, language: Option<&str>,
    path_prefix: Option<&str>, kind: Option<&str>,
    max_results: usize) -> Result<Vec<SearchResult>>
```

Signatures are stored in the index and used by the `signatures` output
mode, giving agents a compact structural overview without reading
function bodies.

## Git Integration

Booger uses tree-sitter to structurally diff branches — not lines,
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
[git integration diagram](doc/architecture.md#git-integration-flow).

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

See the [volatile context diagram](doc/architecture.md#volatile-context-layer).

## Multi-Project Registry

Register projects by name for easy cross-project access:

```bash
booger project add myapp /path/to/myapp
booger project add lib /path/to/lib
booger project list
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

## Architecture

```
Agent (Cursor / Codex / CLI)
  → MCP (JSON-RPC 2.0 over stdio)
    → dispatch (route by method)
      → 18 tool handlers
        → tree-sitter (structural parsing, 7 languages)
        → SQLite + FTS5 (storage, indexing, search, volatile context)
        → git (structural branch diffs)
        → ollama (optional, semantic embeddings)
  → .booger/index.db (single file per project)
```

For detailed visual diagrams of every subsystem, see
**[doc/architecture.md](doc/architecture.md)** — includes Mermaid
diagrams for request flow, tool dispatch, indexing pipeline, search
pipeline, volatile context, git integration, module dependencies,
and the SQLite schema.

## Tech Stack

| Component | Role |
|---|---|
| [Rust](https://www.rust-lang.org/) | Language — performance + single-binary distribution |
| [SQLite](https://sqlite.org/) | Embedded database for all persistent and volatile storage |
| [FTS5](https://www.sqlite.org/fts5.html) | Full-text search with BM25 ranking |
| [Tree-sitter](https://tree-sitter.github.io/tree-sitter/) | Structural code parsing (functions, types, imports) |
| [MCP](https://modelcontextprotocol.io/) | Agent communication protocol (JSON-RPC 2.0 over stdio) |
| [BLAKE3](https://github.com/BLAKE3-team/BLAKE3) | Content hashing for incremental indexing |
| [Ollama](https://ollama.ai/) | Local embedding generation (optional) |

## License

MIT
