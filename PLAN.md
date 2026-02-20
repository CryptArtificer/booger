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

### M0 — Skeleton
- [x] Project init
- [x] CLI scaffold (clap)
- [x] Configuration (index roots, storage path)
- [x] Basic project structure (modules)

### M1 — Ingestion & Storage
- [x] Walk directory tree, respect .gitignore
- [x] Tree-sitter parsing: extract functions, structs, classes, etc.
- [x] Method-level extraction: impl/class/trait/interface blocks decomposed into child methods
- [x] Chunk storage in SQLite (file path, byte range, line range, content, language, kind)
- [x] Incremental updates: hash-based change detection (BLAKE3 content hash)
- [x] `booger index <path>` CLI command

### M2 — Text Search
- [x] Full-text search over stored chunks (SQLite FTS5)
- [x] Path / language / kind filters
- [x] `booger search <query>` CLI command
- [x] Ranked results with context snippets
- [x] JSON output mode for agent consumption
- [x] Auto-index on search: incrementally updates stale/missing index before querying
- [x] Code boost: structural code chunks ranked above raw/doc chunks (+3)
- [x] Chunk size penalty: oversized chunks (READMEs, etc.) penalized (up to -4)
- [x] FTS5 query sanitization: hyphens, dots, slashes auto-quoted
- [ ] Symbol-aware search (find definition, find references)

### M3 — Semantic Search
- [ ] Embedding generation (pluggable backend: local ollama, remote OpenAI)
- [ ] Vector storage and ANN index (usearch or custom HNSW)
- [ ] Hybrid ranking: text score + semantic score
- [ ] `booger semantic <query>` CLI command

### M4 — Volatile Context Layer
- [x] Annotations: attach notes to file/symbol/line-range (with optional TTL)
- [x] Working set: explicit focus paths that boost results
- [x] Visited/blacklist: deprioritize already-seen results
- [x] Search re-ranking using volatile context
- [x] `booger annotate`, `booger focus`, `booger visit`, `booger forget` CLI commands
- [ ] Intents: session-level goals that bias search ranking

### M5 — MCP Server
- [x] MCP protocol implementation (JSON-RPC over stdio)
- [x] Expose tools: search, index, status, annotate, annotations, focus, visit, forget, projects
- [x] Expose resources: indexed project stats
- [x] Agent-friendly structured output
- [x] Multi-project support via `project` parameter

### M6 — Dependency & Structure
- [ ] Import/dependency graph extraction per language
- [ ] "What depends on X?" and "What does X depend on?" queries
- [ ] Directory-level summaries (pre-computed or on-demand)

### M7 — Polish
- [x] Multi-project registry (~/.booger/projects.json)
- [x] Read-only operations (status, search, list, forget) don't create .booger/ as side effect
- [x] Hot-reload proxy script for MCP development (booger-proxy.sh)
- [ ] Filesystem watcher for live re-indexing
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

## Module Structure

```
src/
  main.rs          — CLI entry point (clap)
  lib.rs           — public API / module re-exports
  config.rs        — configuration + project registry
  index/
    mod.rs         — indexing orchestration, auto-index
    walker.rs      — directory traversal + .gitignore
    chunker.rs     — tree-sitter chunking (method-level extraction)
    hasher.rs      — BLAKE3 content hashing
  store/
    mod.rs         — storage abstraction
    sqlite.rs      — SQLite backend (open, open_if_exists, search, stats, volatile context)
    schema.rs      — table definitions + migrations (v3)
  search/
    mod.rs         — query parsing + dispatch
    text.rs        — FTS5 search + code boost + context re-ranking + auto-index
  context/
    mod.rs         — volatile layer orchestration
    annotations.rs — notes attached to code locations (RW/RO split)
    workset.rs     — focus / visited tracking (RW/RO split)
  mcp/
    mod.rs         — MCP server entry point
    server.rs      — JSON-RPC over stdio loop
    protocol.rs    — JSON-RPC + MCP type definitions
    tools.rs       — 9 tool definitions + handlers
    resources.rs   — resource definitions + handlers

Planned:
  search/
    semantic.rs    — vector similarity search (M3)
  embed/
    mod.rs         — embedding abstraction (M3)
  graph/
    mod.rs         — dependency graph (M6)
```

## Non-Goals (for now)

- Web UI
- Real-time collaboration
- Replacing LSP (we complement it, not replace it)
