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
- [x] Hybrid ranking: text score + semantic score
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
- [x] 23 tools: search, grep, references, symbols, workspace-search, hybrid-search, semantic-search, tests-for, directory-summary, changed-since, index, status, embed, annotate, annotations, focus, visit, forget, branch-diff, draft-commit, changelog, batch, projects
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

### M7 — Polish & Quality
- [x] Multi-project registry (~/.booger/projects.json)
- [x] Read-only operations (status, search, list, forget) don't create .booger/ as side effect
- [x] Hot-reload proxy script for MCP development (booger-proxy.sh)
- [x] 77 unit tests across store, tools, protocol, config
- [x] Security hardening: batch caps, thread limits, timestamp validation, no silent fallbacks
- [x] Independent security verification by Codex (live MCP probing)
- [ ] Filesystem watcher for live re-indexing
- [ ] Remote index sharing (optional)
- [ ] Performance tuning (large repos: 100k+ files)
- [ ] Integration tests (end-to-end MCP stdin/stdout)

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
    tools.rs       — 23 tool definitions + handlers
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

- **~~"What changed since I last looked?"~~** ✅ Shipped — `changed-since` takes an
  ISO 8601 timestamp and returns symbols from files re-indexed after that time.

- **Cross-file type flow** — given a type or struct, find all functions that accept
  it, return it, or contain it as a field. This is the structural version of "who
  touches this data?" and would be transformative for understanding unfamiliar
  codebases.

- **~~Batch tool calls~~** ✅ Shipped — `batch` accepts an array of tool calls (max 20)
  and returns all results in one round-trip.

- **~~Auto-index or "index first" guidance~~** ✅ Shipped — When search/references/symbols
  return "no index" or "no indexed files", the message now includes the exact
  command: "Run: booger index \"<path>\"" so agents can suggest it or the user can copy-paste.

- **~~Explain empty results~~** ✅ Shipped — When search/references/symbols return
  0 results, the tool reports a short reason: "No matches.", "Path prefix has no
  indexed files.", "No index found. Run: booger index \"<path>\"" (and similar with path), or "No matches for symbol 'X'."

### Medium — would use often

- **~~Hybrid ranking~~** ✅ Shipped — `hybrid-search` combines FTS + semantic
  scores with tunable alpha (default 0.7 FTS / 0.3 semantic).

- **~~Inline annotations in results~~** ✅ Shipped — `[note]` lines are injected into
  search results for annotated targets; annotations also boost rank by +2.

- **Stale embedding detection** — after re-indexing, some chunks change but their
  embeddings are from the old content. A `embed --stale` or automatic invalidation
  on content change would keep semantic search honest.

- **~~Directory summaries~~** ✅ Shipped — `directory-summary` returns file count,
  languages, symbol kinds, entry points, and subdirectory structure in one call.

- **~~Test association~~** ✅ Shipped — `tests-for` finds tests by naming convention,
  module structure (e.g. Rust `mod tests`), and content (tests that reference the symbol).

- **Search-then-expand** — I often do: search with `files_with_matches` → then batch
  `symbols` (or `references`) for the top N result paths. A single tool that does
  "search X, then return symbols (or references) for the top N matching paths"
  would cut round-trips when I'm exploring ("what's in the files that match X?").

- **~~Scope filter on references~~** ✅ Shipped — `references` accepts optional
  `scope` (definition | call | type | import | reference); returns only that ref kind.

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

- **Session handoff for model switch** — Not core to booger (search/index/working
  memory), but very useful: when the user starts a new chat (new model), there's
  no context. A `handoff` that returns focus + annotations + optional "current
  goal" would let the new agent (or a pasted blob) get up to speed without
  re-reading PLAN, README, and docs every time. Depends on persistent sessions.

## Session handoff / persistent context

**Problem:** When the user switches to a new model (or new chat), there is no
persistent context. Every session we re-do the same dance: read PLAN, README,
docs, re-familiarize. The new agent has no idea what we were working on, what
we focused, or what we annotated. This isn't booger's main goal (search/index
and working memory *within* a session are), but solving it would be very useful.

**What booger could do:**

1. **Handoff document** — A tool or resource (e.g. `handoff` or
   `booger://project/X/context`) that returns a single blob for the project:
   - Current focus paths
   - All annotations (target + note), or for a named session
   - Optional short "current goal" or "session summary" (freeform text the user
     or previous agent can set)
   - Last index time / status one-liner
   - Optionally: top-level directory-summary so "what is this repo" is in one place

   User (or new agent) runs it at the start of a new chat and pastes the result.
   New model gets "here's what we're working on and what we know" without
   re-reading all docs.

2. **Persistent sessions** — Store session state (focus, annotations, optional
   goal/summary) in SQLite keyed by session name. Sessions survive MCP/process
   restart. So the *previous* agent's focus and annotations are still there
   when the *new* agent loads that session or requests the handoff.

3. **Writable "goal" or "summary"** — One field per project or per session that
   the user or agent can set: "Working on: scope filter for references." The
   handoff document includes it so the new agent knows what to do next.

4. **Project briefing (optional)** — A cached one-pager from the index
   (directory-summary at root, entry points, maybe recent branch-diff summary).
   New agent can call `booger briefing` instead of reading PLAN + README + all
   of doc/ to get "what is this project."

**Minimal slice:** A `handoff` tool that returns focus + annotations + (if we
add it) one "goal" line, as markdown or plain text. No new tables if we reuse
annotations + workset; we might add a small `session_goal` or `project_summary`
table. Persistent sessions (survive restart) would make handoff actually useful
across model switches.

## Non-Goals (for now)

- Web UI
- Real-time collaboration
- Replacing LSP (we complement it, not replace it)
