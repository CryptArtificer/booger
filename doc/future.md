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

### "What Changed Since I Last Looked?"
A session-aware diff. When an agent has focused files and another process
modifies them, the agent currently has no way to know. A `changed-since`
tool that takes a timestamp or session start and returns modified symbols
would eliminate stale assumptions.

### Cross-File Type Flow
Given a type or struct, find all functions that accept it, return it, or
contain it as a field. This is the structural version of "who touches
this data?" — transformative for understanding unfamiliar codebases.

### Batch Tool Calls
A `pipeline` or `multi` tool that accepts an array of tool calls and
returns all results in one round trip. Agents often need "search X AND
get symbols for the top 3 result files" — today that's 4 calls, could
be 1.

## Medium Priority

### Stale Embedding Detection
After re-indexing, some chunks change but their embeddings are from the
old content. Either `embed --stale` for explicit refresh, or automatic
invalidation when a chunk's content hash changes.

### Directory Summaries
`symbols` on a directory gives every symbol in every file. What agents
often want is a higher-level view: "12 files, 47 functions, 8 structs,
main responsibilities: X, Y, Z". Pre-computed or on-demand.

### Test Association
Given a function, find its tests (by naming convention, proximity, or
import graph). Given a test, find what it tests. Trivial for humans,
hard for agents without structural support.

### Persistent Sessions
Sessions are currently just string labels with no persistence across
restarts. Named sessions stored in SQLite would let agents resume context
across conversation boundaries.

## Longer Term

### Workspace-Level Search
Search across all registered projects at once, with results tagged by
project. Useful for monorepo-adjacent setups where related code lives
in sibling repos.

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
