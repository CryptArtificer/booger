# Booger — Project Plan

> "I found it!" — Ralph Wiggum
>
> A local code search engine, index, and working memory for AI agents.

## Vision

Booger is a tool that AI agents (via MCP, CLI, or API) use to efficiently
search, navigate, and annotate local codebases. It combines persistent
code indexing with volatile session context to act as a shared working
memory.

## Architecture

```
┌─────────────────────────────────────────────────┐
│                  Agent (MCP / CLI)               │
├─────────────────────────────────────────────────┤
│                   Query Engine                   │
│         (semantic + structural + volatile)        │
├──────────────────┬──────────────────────────────┤
│  Persistent Layer │     Volatile Layer           │
│  ─────────────── │     ──────────────           │
│  Code chunks      │     Annotations (TTL)        │
│  Symbol index     │     Intents / goals          │
│  Dependency graph  │     Working set / focus      │
│  Embeddings       │     Visited / blacklist       │
├──────────────────┴──────────────────────────────┤
│                   Storage                        │
│           SQLite + vector index                  │
├─────────────────────────────────────────────────┤
│                  Ingestion                       │
│    Tree-sitter chunking + incremental updates    │
│    Filesystem watcher / git-diff based           │
└─────────────────────────────────────────────────┘
```

## Milestones

### M0 — Skeleton (current)
- [x] Project init
- [ ] CLI scaffold (clap)
- [ ] Configuration (index roots, storage path)
- [ ] Basic project structure (modules)

### M1 — Ingestion & Storage
- [ ] Walk directory tree, respect .gitignore
- [ ] Tree-sitter parsing: extract functions, structs, classes, etc.
- [ ] Chunk storage in SQLite (file path, byte range, line range, content, language, kind)
- [ ] Incremental updates: hash-based change detection (mtime + content hash)
- [ ] `booger index <path>` CLI command

### M2 — Text Search
- [ ] Full-text search over stored chunks (SQLite FTS5 or tantivy)
- [ ] Symbol-aware search (find definition, find references)
- [ ] Path / language / kind filters
- [ ] `booger search <query>` CLI command
- [ ] Ranked results with context snippets

### M3 — Semantic Search
- [ ] Embedding generation (pluggable backend: local ollama, remote OpenAI)
- [ ] Vector storage and ANN index (usearch or custom HNSW)
- [ ] Hybrid ranking: text score + semantic score
- [ ] `booger semantic <query>` CLI command

### M4 — Volatile Context Layer
- [ ] Annotations: attach notes to file/symbol/line-range (with optional TTL)
- [ ] Intents: session-level goals that bias search ranking
- [ ] Working set: explicit focus paths that boost results
- [ ] Visited/blacklist: deprioritize already-seen results
- [ ] `booger annotate`, `booger focus`, `booger forget` CLI commands

### M5 — MCP Server
- [ ] MCP protocol implementation (JSON-RPC over stdio)
- [ ] Expose tools: search, semantic, annotate, focus, index
- [ ] Expose resources: indexed projects, stats
- [ ] Agent-friendly structured output

### M6 — Dependency & Structure
- [ ] Import/dependency graph extraction per language
- [ ] "What depends on X?" and "What does X depend on?" queries
- [ ] Directory-level summaries (pre-computed or on-demand)

### M7 — Polish
- [ ] Filesystem watcher for live re-indexing
- [ ] Multi-project support
- [ ] Remote index sharing (optional)
- [ ] Performance tuning (large repos: 100k+ files)
- [ ] Comprehensive error handling and logging

### M8 — Cloud Deployment
- [ ] HTTP/gRPC API server mode (`booger serve`) for remote access
- [ ] AWS deployment: Lambda + API Gateway for serverless, or ECS/Fargate for persistent
- [ ] S3-backed storage adapter (index DB + embeddings)
- [ ] SQS/EventBridge for async indexing jobs (large repos)
- [ ] IAM-based auth for multi-tenant access
- [ ] CloudFormation / CDK template for one-click deploy
- [ ] Provider abstraction layer: trait-based storage/queue/auth backends
- [ ] Azure extension: Blob Storage + Azure Functions
- [ ] GCP extension: Cloud Storage + Cloud Run
- [ ] Terraform modules as alternative to provider-native IaC

## Key Design Decisions

1. **Rust** — performance matters for indexing large repos; also good for
   single-binary distribution.

2. **SQLite as primary store** — simple, embedded, portable. One `.booger`
   directory per indexed project (or a central one).

3. **Tree-sitter for chunking** — language-aware boundaries mean search
   results are logical units (functions, types), not arbitrary line ranges.

4. **Pluggable embeddings** — local-first (ollama) with optional remote
   backends. Embedding model is stored in index metadata so results are
   reproducible.

5. **Two-layer model** — persistent index + volatile context. Search
   queries hit both and results are merged/re-ranked.

6. **MCP-first API** — designed for agent consumption. CLI is a thin
   wrapper over the same core.

7. **Cloud-ready architecture** — core logic is storage-agnostic via traits.
   Local uses SQLite + filesystem, cloud swaps in S3 + managed DB. AWS is
   the primary target; other providers via the same trait abstraction.

## Module Structure (planned)

```
src/
  main.rs          — CLI entry point (clap)
  lib.rs           — public API / core orchestration
  config.rs        — configuration loading
  index/
    mod.rs         — indexing orchestration
    walker.rs      — directory traversal + .gitignore
    chunker.rs     — tree-sitter chunking
    hasher.rs      — content hashing for incremental updates
  store/
    mod.rs         — storage abstraction
    sqlite.rs      — SQLite backend
    schema.rs      — table definitions + migrations
  search/
    mod.rs         — query parsing + dispatch
    text.rs        — full-text search (FTS5 / tantivy)
    semantic.rs    — vector similarity search
    ranking.rs     — hybrid ranking / result merging
  embed/
    mod.rs         — embedding abstraction
    ollama.rs      — local ollama backend
    openai.rs      — OpenAI backend
  context/
    mod.rs         — volatile layer orchestration
    annotations.rs — notes attached to code locations
    intent.rs      — session-level goals
    workset.rs     — focus / visited tracking
  mcp/
    mod.rs         — MCP server implementation
    tools.rs       — tool definitions
    resources.rs   — resource definitions
  graph/
    mod.rs         — dependency graph
    extract.rs     — import extraction per language
    query.rs       — graph traversal queries
```

## Non-Goals (for now)

- Web UI
- Real-time collaboration
- Replacing LSP (we complement it, not replace it)
