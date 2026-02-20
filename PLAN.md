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
│                 Agent (MCP / CLI)               │
├─────────────────────────────────────────────────┤
│                  Query Engine                   │
│       (semantic + structural + volatile)        │
├──────────────────┬──────────────────────────────┤
│ Persistent Layer │     Volatile Layer           │
│  ─────────────── │     ──────────────           │
│ Code chunks      │     Annotations (TTL)        │
│ Symbol index     │     Intents / goals          │
│ Dependency graph │     Working set / focus      │
│ Embeddings       │     Visited / blacklist      │
├──────────────────┴──────────────────────────────┤
│                   Storage                       │
│           SQLite + vector index                 │
├─────────────────────────────────────────────────┤
│                  Ingestion                      │
│    Tree-sitter chunking + incremental updates   │
│    Filesystem watcher / git-diff based          │
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
- [x] Embedding generation via local Ollama (nomic-embed-text, 768d)
- [x] Vector storage in SQLite (f32 BLOBs, cosine similarity)
- [ ] Hybrid ranking: text score + semantic score
- [x] `booger semantic <query>` CLI command
- [x] MCP tool: `embed` and `semantic-search`

### M4 — Volatile Context Layer
- [x] Annotations: attach notes to file/symbol/line-range (with optional TTL)
- [x] Working set: explicit focus paths that boost results
- [x] Visited/blacklist: deprioritize already-seen results
- [x] Search re-ranking using volatile context
- [x] `booger annotate`, `booger focus`, `booger visit`, `booger forget` CLI commands
- [ ] Intents: session-level goals that bias search ranking

### M5 — MCP Server
- [x] MCP protocol implementation (JSON-RPC over stdio)
- [x] 16 tools: search, index, status, annotate, annotations, focus, visit, forget, branch-diff, draft-commit, changelog, projects, embed, semantic-search, symbols, grep
- [x] Expose resources: indexed project stats
- [x] Agent-friendly structured output (content, files_with_matches, signatures, count)
- [x] Multi-project support via `project` parameter
- [x] Pagination: `head_limit` + `offset` across all listing tools
- [x] Content truncation: `max_lines` for content mode
- [x] Smart signatures: tree-sitter-extracted function/struct/trait declarations (params + return types, no body)
- [x] Kind filtering: `kind` parameter on search, symbols, grep
- [x] Protocol hardening: `-32600` vs `-32700` error codes, strict resource URI validation, unknown project rejection, safe serialization (no panics)

### M5.1 — Git Integration
- [x] `branch-diff`: structural diff between branches (added/modified/removed symbols per file)
- [x] `draft-commit`: auto-generate commit messages from staged/unstaged structural changes
- [x] `changelog`: generate markdown changelog from branch diff (grouped by Added/Modified/Removed)
- [x] Auto-focus changed files from branch-diff (`--focus`)
- [x] Import/use statement indexing: `use` (Rust), `import`/`from`/`require()` (JS/TS/Python), `import` (Go), `#include` (C)
- [x] Robust git output parsing: `-z` NUL terminator, `--no-renames`, duplicate symbol handling
- [x] CLI + MCP tool exposure for all git commands

### M6 — Dependency & Structure
- [x] Import/use statement indexing (moved to M5.1)
- [ ] Full dependency graph extraction per language (beyond imports)
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
    schema.rs      — table definitions + migrations (v5)
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
    tools.rs       — 16 tool definitions + handlers
    resources.rs   — resource definitions + handlers
  git/
    mod.rs         — git integration entry point
    diff.rs        — structural branch diff, staged diff (tree-sitter chunk comparison)
    format.rs      — commit message + changelog generation from structural diffs
  search/
    semantic.rs    — vector similarity search (cosine over embedded chunks)
  embed/
    mod.rs         — embedding trait + cosine_similarity
    ollama.rs      — Ollama HTTP client for embedding generation

Planned:
  graph/
    mod.rs         — dependency graph (M6)
```

## Agent Wishlist

Things I (the agent) actually want, based on daily use:

### High — would use every session

- **"Where is this called?"** — given a function name, find all call sites across
  the index. Not grep — structural. Tree-sitter can find call expressions and match
  the callee name. This is the #1 thing I waste round-trips on today: grepping,
  filtering false positives from comments/strings, re-reading to confirm.

- **Scope-aware search** — when I search for `dispatch`, I want results ranked by
  whether the match is in a function name, a call site, a string literal, or a
  comment. Right now they're all equal. A `scope` filter (`definition`, `call`,
  `reference`, `comment`) would save me from reading irrelevant hits.

- **"What changed since I last looked?"** — a session-aware diff. I annotate files
  via `focus`, I work on them, but if someone else (or I in another session) edits
  a focused file, I have no way to know. A `changed-since` tool that takes a
  timestamp or session start and returns modified symbols would eliminate stale
  assumptions.

- **Cross-file type flow** — given a type or struct, find all functions that accept
  it, return it, or contain it as a field. This is the structural version of "who
  touches this data?" and would be transformative for understanding unfamiliar
  codebases.

- **Batch tool calls** — MCP lets the client batch, but from the agent side I often
  want to say "search X AND get symbols for the top 3 result files" in one round
  trip. A `pipeline` or `multi` tool that accepts an array of tool calls and returns
  all results would cut my latency in half for exploratory workflows.

### Medium — would use often

- **Hybrid ranking** — combine FTS + semantic scores. Right now I have to choose
  between `search` (keyword) and `semantic-search` (meaning). A single query that
  blends both would give me the best of both without two round-trips.

- **Inline annotations in results** — when I've annotated `src/mcp/server.rs:51`
  with "dispatch entry point — check error handling", I want that note to appear
  *in* search results that include that line range. Right now annotations exist
  in a separate silo and I have to remember to check them.

- **Stale embedding detection** — after re-indexing, some chunks change but their
  embeddings are from the old content. A `embed --stale` or automatic invalidation
  on content change would keep semantic search honest.

- **Directory summaries** — `symbols` on a directory gives me every symbol in every
  file. What I often want is a higher-level view: "this directory has 12 files, 47
  functions, 8 structs, main responsibilities appear to be X, Y, Z". Pre-computed
  or on-demand directory-level summaries would help me orient faster in large
  codebases.

- **Test association** — given a function, find its tests (by naming convention,
  proximity, or import graph). Given a test, find what it tests. This is trivial
  for humans ("it's right below") but hard for agents without structural support.

### Low — nice to have

- [x] **Workspace-level search** — `workspace-search` tool: searches all registered
  projects in parallel (threaded), merges results ranked globally, tags each hit
  with its project name. Supports all output modes, pagination, and filters.

- **Diff-aware search boost** — if I'm on a branch with changes, automatically
  boost search results from changed files (like auto-focus, but implicit and
  always-on). The branch-diff + focus combo works today but requires explicit
  invocation.

- **Undo for volatile context** — I sometimes focus the wrong path or add a bad
  annotation. A simple undo/history for context mutations would save cleanup time.

- **Streaming results** — for large result sets, stream results as they're found
  rather than buffering everything. Particularly valuable for `grep` on large
  indexes where I only need the first few matches.

- **Persistent sessions** — right now sessions are just string labels with no
  persistence across restarts. Named sessions that survive server restarts (stored
  in SQLite) would let me resume context across conversation boundaries.

## Non-Goals (for now)

- Web UI
- Real-time collaboration
- Replacing LSP (we complement it, not replace it)
