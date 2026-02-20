# Booger

> "I found it!" — Ralph Wiggum

A local code search engine, index, and working memory for AI agents.

Booger indexes your codebases using tree-sitter for structural chunking,
stores everything in SQLite with FTS5, and exposes it all via MCP or CLI.
It's designed to be the tool that agents use to efficiently find and
reason about code.

One binary, zero dependencies, zero background processes.

## Install

```bash
cargo install --path .
```

## Quick Start

```bash
# Search a project (auto-indexes if needed)
booger search "parse config" --language rust

# Explicit indexing (incremental — only re-processes changed files)
booger index /path/to/project

# JSON output (for scripts/agents)
booger search "hash file" --json
```

Searching auto-indexes: if the index is missing or stale, booger
incrementally updates it before returning results. You never need
to manually run `booger index`.

## Multi-Project

Register projects by name for easy access:

```bash
booger project add myapp /path/to/myapp
booger project add lib /path/to/lib
booger project list

# Search a specific project
booger search "auth" --root /path/to/myapp
```

## MCP Server

Booger runs as an MCP tool server for AI agent integration:

```bash
booger mcp /path/to/project
```

### Cursor Configuration

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

### Development Setup (hot-reload)

For active development, use the proxy script so `cargo install`
takes effect immediately without restarting the MCP session:

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
so rebuilding and installing picks up changes instantly.

### Available MCP Tools

| Tool | Description |
|---|---|
| `search` | Full-text search with language/path filters and context-aware ranking |
| `index` | Index a directory (incremental) |
| `status` | Index stats (files, chunks, languages, sizes) |
| `annotate` | Attach notes to files/symbols with optional TTL |
| `annotations` | List annotations |
| `focus` | Boost search results for specific paths |
| `visit` | Deprioritize already-seen paths in search |
| `forget` | Clear volatile context |
| `projects` | List registered projects |

All tools accept an optional `project` parameter — a registered project
name or a literal path.

## How Search Works

```
Query
  → auto-index (walk + BLAKE3 hash, skip unchanged files)
  → FTS5 full-text search (Porter stemmer, BM25 ranking)
  → static re-ranking:
      code chunks boosted over docs/raw (+3)
      oversized chunks penalized (up to -4)
  → volatile context re-ranking:
      focused paths boosted (+5)
      visited paths penalized (-3)
      annotated targets boosted (+2)
  → return top N results
```

Search results are individual functions, structs, and classes —
not entire files. Container blocks (impl, class, trait) are split
into their child methods so you get precisely the code you need.

Read-only operations (status, search on indexed project, annotations
list, forget) never create a `.booger/` directory as a side effect.

## Volatile Context

Beyond static indexing, booger maintains a volatile context layer:

- **Annotations**: attach notes to files, symbols, or line ranges.
  Supports session scoping and TTL for auto-expiry.
- **Focus**: mark paths as focused to boost their search results.
- **Visited**: mark paths as visited to deprioritize them.

This turns booger from a search engine into a working memory — agents
can accumulate knowledge during a session and have it influence future
queries.

```bash
# Annotate a file
booger annotate src/parser.rs "Has a known bug in error recovery"

# Focus on an area of the codebase
booger focus src/mcp src/search

# Mark files as already reviewed
booger visit src/config.rs

# Clear everything
booger forget
```

## Supported Languages

Tree-sitter structural chunking (functions, structs, classes, methods):

Rust, Python, JavaScript, TypeScript, TSX, Go, C

Container blocks (impl, class, trait, interface) are decomposed into
individual method chunks plus a signature-only chunk for the container.

All other file types are indexed as whole-file chunks and are still
searchable via FTS5.

## Configuration

Run `booger init` to create a `.booger/config.toml`:

```toml
[storage]
max_size_bytes = 0  # 0 = unlimited

[resources]
max_threads = 6     # 0 = half available cores
max_memory_bytes = 268435456  # 256MB
batch_size = 500

[embed]
type = "none"       # "ollama" or "openai" when semantic search is ready
```

## Architecture

```
Cursor (or any MCP client)
  → MCP (JSON-RPC over stdio)
    → booger-proxy.sh (optional, for dev hot-reload)
      → booger mcp (Rust binary)
        → tree-sitter (parse code into function-level chunks)
        → SQLite + FTS5 (store, index, search, volatile context)
          → .booger/index.db (single file, lives next to your repo)
```

## Data Flow

```
Files on disk
  → index (walk + hash + tree-sitter parse → SQLite + FTS5)
  → search (FTS5 query → BM25 → code boost → context re-rank)
  → volatile context (annotations, focus, visited — session-scoped)
  → forget (cleanup)
```

Everything is a single SQLite file per project. No external services,
no background processes, no daemon. Each MCP request is a fresh process
invocation — stateless from the OS perspective, stateful from the data
perspective.

## License

MIT
