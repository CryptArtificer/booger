# Booger

<p align="center">
  <img src="doc/rw-booger.png" width="400" alt="Ralph Wiggum picking his nose" title="Named after the only thing Ralph Wiggum is genuinely, undeniably good at: finding stuff nobody asked him to look for. AI agents are basically Ralph — wandering around the codebase, poking at things, getting distracted, occasionally eating paste. But every now and then they pull out something brilliant and hold it up proudly: 'I found it!' Booger just makes sure they find it on the first try instead of the forty-seventh." />
</p>

<p align="center">
  <strong>A local code search engine, index, and working memory for AI agents.</strong>
</p>

<p align="center">
  One binary &middot; Zero dependencies &middot; Zero background processes &middot; 14 MB
</p>

---

Booger indexes your codebases using [Tree-sitter](https://tree-sitter.github.io/tree-sitter/)
for structural chunking, stores everything in [SQLite](https://sqlite.org/) with
[FTS5](https://www.sqlite.org/fts5.html), and exposes it all via
[MCP](https://modelcontextprotocol.io/) or CLI. It's designed to be the tool that
AI agents use to efficiently find and reason about code.

23 tools. 7 languages. 77 tests. Structural search, references, git-aware diffs,
semantic embeddings, volatile working memory, batch calls, test discovery — all in
a single static binary.

**Average 20,514x token reduction** vs raw `rg` across real projects.
[See the benchmarks.](doc/search.md#measured-token-savings)

## Documentation

| | |
|---|---|
| **[Search & Benchmarks](doc/search.md)** | How search works, token savings, performance numbers |
| **[User Guide](doc/guide.md)** | Configuration, languages, semantic search, git, volatile context |
| **[Security](doc/security.md)** | Threat model, hardening, verified properties |
| **[Architecture](doc/architecture.md)** | Mermaid diagrams of every subsystem |
| **[Concepts](doc/concepts.md)** | FTS5, Tree-sitter, MCP, BM25, BLAKE3, embeddings |
| **[Live Demo](doc/demo.md)** | 11-step walkthrough of an agent tracing a call chain |
| **[Future Additions](doc/future.md)** | Roadmap and planned features |
| **[Project Plan](PLAN.md)** | Milestones and design decisions |

## Install

Requires [Rust 1.75+](https://rustup.rs/):

```bash
git clone https://github.com/CryptArtificer/booger.git
cd booger
make install    # or: cargo install --path .
```

Verify:

```bash
booger --version
booger status
```

## Quick Start

```bash
booger index /path/to/project           # incremental — only changed files
booger search "parse config"            # auto-indexes if needed
booger symbols src/main.rs              # structural outline
booger references dispatch              # find all call sites
booger branch-diff main                 # symbol-level diff
booger draft-commit                     # auto-generated commit message
```

## MCP Server

Booger runs as an [MCP](https://modelcontextprotocol.io/) server for AI agents.
MCP is JSON-RPC 2.0 over stdio — no HTTP, no daemon.

```bash
booger mcp /path/to/project
```

### Cursor

`.cursor/mcp.json`:

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

```bash
codex --mcp "booger:booger mcp /path/to/project"
```

### Development (hot-reload)

Use the proxy script so `cargo install` picks up immediately without
restarting the MCP session:

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

## Available MCP Tools

| Tool | Description |
|---|---|
| | **Search & Discovery** |
| `search` | Full-text search with [BM25](https://en.wikipedia.org/wiki/Okapi_BM25) ranking and volatile context re-ranking |
| `grep` | Regex/literal search within indexed chunks — matching lines with context |
| `references` | Find all usages of a symbol: definitions, call sites, type refs, imports |
| `symbols` | Structural outline of a file/directory with smart signatures |
| `workspace-search` | Search all registered projects at once (threaded) |
| `hybrid-search` | Combined FTS + semantic search with tunable weighting |
| `semantic-search` | Similarity search via local embeddings ([Ollama](https://ollama.ai/)) |
| `tests-for` | Find test functions for a symbol by naming, module structure, content |
| `directory-summary` | File count, languages, symbol kinds, entry points, subdirectories |
| `changed-since` | Symbols from files re-indexed after a timestamp |
| | |
| | **Indexing & Embeddings** |
| `index` | Index a directory (incremental, [BLAKE3](https://github.com/BLAKE3-team/BLAKE3) change detection) |
| `status` | Index stats: files, chunks, languages, chunk kind breakdown |
| `embed` | Generate embeddings via [Ollama](https://ollama.ai/) |
| | |
| | **Volatile Context** |
| `annotate` | Attach notes to files/symbols/lines with optional TTL |
| `annotations` | List annotations (filterable by target and session) |
| `focus` | Boost search results for specific paths |
| `visit` | Deprioritize already-seen paths |
| `forget` | Clear volatile context (all or session-scoped) |
| | |
| | **Git** |
| `branch-diff` | Structural diff between branches — added/modified/removed symbols |
| `changelog` | Markdown changelog from branch diff |
| `draft-commit` | Commit message from staged/unstaged structural changes |
| | |
| | **Multi-tool** |
| `batch` | Execute multiple tool calls in one round-trip (max 20) |
| | |
| | **Registry** |
| `projects` | List registered projects |

All tools accept an optional `project` parameter. Unknown project names
return an error (no silent fallback).

**Output modes:** `content` (default), `signatures`, `files_with_matches`, `count`.
Plus `head_limit`/`offset` for pagination and `max_lines` to cap output.
[Details in the search docs.](doc/search.md#output-modes)

## Security

Booger is designed for **local, trusted developer environments** — CLI
and MCP over stdio. No network listener, no daemon.

**Verified properties** (independently stress-tested by Codex):

- Parameterized SQL (no injection), explicit git args (no shell interpolation)
- Batch calls capped at 20, workspace-search threads capped at 10
- Timestamp validation, strict project/URI resolution, correct JSON-RPC error codes
- No `unwrap()` in MCP paths, read-only ops never create files
- 77 unit tests across store, tools, protocol, and config
- Stable under concurrent load (60+ parallel requests, 0 failures)

**Not in scope** (local tool): auth, tenant isolation, TLS, transport rate limiting.

[Full security details →](doc/security.md)

## Testing

```bash
make test       # or: cargo test
```

77 tests across 4 modules:

| Module | Tests | Coverage |
|---|---|---|
| `store/sqlite` | 36 | CRUD, FTS search (filters, signatures), annotations, workset, embeddings, transactions, FTS sanitization, changed-since |
| `mcp/tools` | 28 | Dispatch, batch (limits, recursion), timestamp validation, directory-summary, tests-for, grep, references, format opts |
| `mcp/protocol` | 11 | JSON-RPC request/response, serialization, tool results, notifications |
| `config` | 10 | Load/save, defaults, registry CRUD, resolve |

## Architecture

```
Agent (Cursor / Codex / CLI)
  → MCP (JSON-RPC 2.0 over stdio)
    → 23 tool handlers
      → Tree-sitter (7 languages)
      → SQLite + FTS5
      → git (structural diffs)
      → Ollama (optional embeddings)
  → .booger/index.db (one file per project)
```

[Full architecture diagrams →](doc/architecture.md)

## Tech Stack

| Component | Role |
|---|---|
| [Rust](https://www.rust-lang.org/) | Performance + single-binary distribution |
| [SQLite](https://sqlite.org/) | All persistent and volatile storage |
| [FTS5](https://www.sqlite.org/fts5.html) | Full-text search with BM25 ranking |
| [Tree-sitter](https://tree-sitter.github.io/tree-sitter/) | Structural code parsing |
| [MCP](https://modelcontextprotocol.io/) | Agent protocol (JSON-RPC 2.0 over stdio) |
| [BLAKE3](https://github.com/BLAKE3-team/BLAKE3) | Content hashing for incremental indexing |
| [Ollama](https://ollama.ai/) | Local embedding generation (optional) |

## License

MIT
