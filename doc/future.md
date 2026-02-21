# Future Additions

Ideas and planned features, roughly ordered by expected impact.

## Likely Next

### Filesystem Watcher
Live monitoring of indexed directories using OS-level file events
(`inotify` on Linux, `FSEvents` on macOS, `ReadDirectoryChangesW` on
Windows). The Rust [`notify`](https://docs.rs/notify/) crate provides a
cross-platform abstraction.

**Tradeoff:** This requires a background process, which contradicts
booger's current "zero background processes" design. Could be opt-in
via `booger watch /path` or a config flag. The auto-index-on-search
pattern (~50ms no-change check) is currently fast enough for most
workflows.

### Scope-Aware Search
When searching for a symbol name, rank results by *where* the match
occurs: function name > call site > type position > string literal >
comment. A `scope` filter (`definition`, `call`, `reference`, `comment`)
would let agents skip irrelevant hits entirely.

### ~~"What Changed Since I Last Looked?"~~ ✅ Shipped
Implemented as the `changed-since` MCP tool. Takes an ISO 8601 timestamp
and returns all symbols from files re-indexed after that time.

### Cross-File Type Flow
Given a type or struct, find all functions that accept it, return it, or
contain it as a field. This is the structural version of "who touches
this data?" — transformative for understanding unfamiliar codebases.

### ~~Batch Tool Calls~~ ✅ Shipped
Implemented as the `batch` MCP tool. Accepts an array of `{tool, arguments}`
objects and returns all results in a single round-trip.

## Medium Priority

### Stale Embedding Detection
After re-indexing, some chunks change but their embeddings are from the
old content. Either `embed --stale` for explicit refresh, or automatic
invalidation when a chunk's content hash changes.

### ~~Directory Summaries~~ ✅ Shipped
Implemented as the `directory-summary` MCP tool. Returns file count,
languages, symbol kind breakdown, entry points, and subdirectory
structure in a single call.

### ~~Test Association~~ ✅ Shipped
Implemented as the `tests-for` MCP tool. Finds tests by naming
convention, module structure (Rust `mod tests`), and content analysis
(test functions that reference the symbol).

### Persistent Sessions
Sessions are currently just string labels with no persistence across
restarts. Named sessions stored in SQLite would let agents resume context
across conversation boundaries.

## Longer Term

### ~~Workspace-Level Search~~ ✅ Shipped
Implemented as the `workspace-search` MCP tool. Searches all registered
projects at once, results tagged by project name. Supports all output
modes, pagination, and kind/language filters.

### Diff-Aware Search Boost
If on a branch with changes, automatically boost search results from
changed files — like auto-focus, but implicit and always-on.

### Full Dependency Graph
Beyond import indexing: resolve imports to actual files/modules, build
a proper graph, and answer "What depends on X?" and "What does X depend
on?" queries.

### HTTP/gRPC Server Mode
`booger serve` for remote access. Enables shared indexes across a team
or CI pipeline.

### Cloud Deployment
- AWS: Lambda + API Gateway, S3-backed storage, SQS for async indexing
- Provider abstraction layer for Azure/GCP portability
- CloudFormation/CDK templates

### Language Server Protocol (LSP) Bridge
Expose booger's index as an LSP server for editor integration beyond
MCP-capable clients. Not a replacement for language-specific LSPs,
but a complement for cross-language search and navigation.

### Undo for Volatile Context
History for context mutations (focus, visit, annotate). Agents sometimes
focus the wrong path or add a bad annotation — a simple undo would save
cleanup time.

### Streaming Results
For large result sets, stream results as they're found rather than
buffering everything. Particularly valuable for `grep` on large indexes
where only the first few matches are needed.
