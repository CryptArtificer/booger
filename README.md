# Booger

> "I found it!" — Ralph Wiggum

A local code search engine, index, and working memory for AI agents.

Booger indexes your codebases using tree-sitter for structural chunking,
stores everything in SQLite with FTS5, and exposes it all via MCP or CLI.
It's designed to be the tool that agents use to efficiently find and
reason about code.

## Install

```bash
cargo install --path .
```

## Quick Start

```bash
# Index a project
booger index /path/to/project

# Search for code
booger search "parse config" --language rust

# JSON output (for scripts/agents)
booger search "hash file" --json
```

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

Tree-sitter structural chunking (functions, structs, classes, etc.):

Rust, Python, JavaScript, TypeScript, TSX, Go, C

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
Agent (MCP / CLI)
    │
    ▼
Query Engine (FTS5 + volatile context re-ranking)
    │
    ├── Persistent Layer: code chunks, symbol index, embeddings
    │       └── SQLite (WAL mode, FTS5, auto-synced triggers)
    │
    └── Volatile Layer: annotations, focus/visited, intents
            └── SQLite (same DB, session-scoped)
    │
    ▼
Ingestion (tree-sitter chunking, BLAKE3 hashing, incremental)
```

## License

MIT
