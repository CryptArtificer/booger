# Search & Benchmarks

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

If an AND-style query returns no results, booger automatically retries
with OR between terms so at least partial matches surface.

Read-only operations (`status`, `search`, `annotations`, `forget`) never
create a `.booger/` directory as a side effect.

See the [search pipeline diagram](architecture.md) for a visual breakdown.

## References

The `references` tool goes further: given a symbol name, it finds every
chunk that mentions it and classifies each hit as `[definition]`,
`[call]`, `[type]`, `[import]`, or `[reference]`, along with which
function the usage lives in. Optional **scope** filter: pass
`scope: "call"` (or `definition`, `type`, `import`, `reference`) to
return only that ref kind — e.g. "only call sites for symbol X".

## Hybrid Search

`hybrid-search` runs both FTS and semantic search, normalizes scores
to [0,1], and merges with configurable alpha (default 0.7 FTS / 0.3
semantic). Degrades gracefully when embeddings aren't available.

## Workspace Search

`workspace-search` queries **all registered projects** in one call
(threaded, capped at 10 concurrent threads). Results are tagged with
the project name and ranked globally. Supports all the same output
modes, pagination, and filters as `search`.

## Batch Tool Calls

`batch` accepts an array of tool calls (max 20) and returns all results
in a single round-trip. Eliminates the "search → get symbols → get
references" 3-call pattern down to 1.

## Test Discovery

`tests-for` finds test functions associated with a symbol:

- **Naming convention:** `test_<name>`, `<name>_test`, `<Name>Test`
- **Module structure:** functions inside `mod tests` blocks (Rust)
- **Content analysis:** test functions that call or reference the symbol

## Changed-Since Detection

`changed-since` returns symbols from files re-indexed after a given
ISO 8601 timestamp. Validates timestamp format and rejects bad input.
Answers "what changed since I last looked?" without diffing entire files.

## Directory Summaries

`directory-summary` gives a high-level overview in one call: file count,
languages, symbol kind breakdown, entry points (`main`, `run`,
`handle_*`, `cmd_*`, `new`), and subdirectory structure.

## Smart Signatures

When booger indexes code, it extracts clean type signatures using
Tree-sitter — everything up to the function body, without the `{`:

```
pub fn search(&self, query: &str, language: Option<&str>,
    path_prefix: Option<&str>, kind: Option<&str>,
    max_results: usize) -> Result<Vec<SearchResult>>
```

Signatures are stored in the index and used by the `signatures` output
mode, giving agents a compact structural overview without reading
function bodies.

## Output Modes

Search tools support multiple output modes to minimize token usage:

| Mode | Use case | Example |
|---|---|---|
| `content` | Full code with line numbers (default) | Function body with `[note]` annotations |
| `signatures` | One-line smart signatures | `fn search(&self, query: &str, ...) -> Result<Vec<SearchResult>>` |
| `files_with_matches` | Just file paths and line ranges | `src/store/sqlite.rs:209:280 [function] search` |
| `count` | Just the number | `42 result(s)` |

Additional controls: `head_limit` / `offset` for pagination, `max_lines`
to cap content output, `kind` to filter by chunk type.

## Explain empty results

When `search`, `references`, or `symbols` return 0 results, the tool reports
a short reason so agents know what to do next:

| Reason | Meaning |
|---|---|
| **No matches.** | Index exists and has chunks; the query or filters matched nothing. |
| **No index found. Run: booger index \"…\"** | No database at the project's storage path. Message includes the exact path to run. |
| **No indexed files. Run: booger index \"…\"** | Database exists but has no chunks. Message includes the exact path to run. |
| **Path prefix has no indexed files. Run: booger index \"…\"** | Path prefix was given and has no indexed files. Message includes the project path. |
| **No matches for symbol 'X'.** | (`references` only) Index has chunks but no definition or reference for that symbol. |

This avoids the "empty result with no explanation" case and helps agents suggest
indexing, broadening the query, or changing the path.

## Why Booger?

AI agents spend most of their tokens **finding** code, not **writing** it.

| Step | Without booger | With booger |
|---|---|---|
| Find code across 5 repos | `rg` / `find` × 5 calls, ~10,000–25,000 tokens of raw output | `workspace-search` — 1 call, ~190 tokens |
| Understand module structure | Read entire files, ~2,000 tokens each | `symbols` — one-line signatures, ~200 tokens |
| Find who calls a function | `rg` + manually filter definitions, comments, strings | `references` — classified as definition/call/import/type |
| Check what changed on a branch | `git diff` — raw line diff, thousands of tokens | `branch-diff` — symbol-level: added/modified/removed |
| Generate a commit message | Read the diff, reason about it | `draft-commit` — structural summary, done |

## Measured Token Savings

Benchmark: `workspace-search` (signatures mode, 10 results) vs `rg` across
5 real projects (664 files, 4,840 chunks). Both tools search the same
source files — `rg` excludes `node_modules`, `target`, `.git`, binaries,
and lock files for a fair comparison. Tokens estimated at ~4 chars/token.

| Query | `rg` output | `rg` est. tokens | booger output | booger est. tokens | Reduction |
|---|---|---|---|---|---|
| `parse` | 123,148 chars | ~30,787 | 1,136 chars | ~284 | **108x** |
| `error` | 150,980 chars | ~37,745 | 900 chars | ~225 | **168x** |
| `config` | 119,363 chars | ~29,840 | 1,222 chars | ~305 | **98x** |
| `search` | 1,201,373 chars | ~300,343 | 1,290 chars | ~322 | **933x** |
| `dispatch` | 4,891 chars | ~1,222 | 1,253 chars | ~313 | **4x** |
| `schema` | 48,905 chars | ~12,226 | 1,128 chars | ~282 | **43x** |
| `test` | 190,986 chars | ~47,746 | 1,333 chars | ~333 | **143x** |

**Average: 214x reduction. Median: 108x.** These are honest numbers —
`rg` is already filtered to source files only. The reduction comes from
booger returning structured signatures instead of raw matching lines.

For rare terms like `dispatch` (only 4x), rg already returns little.
For common terms like `search` (933x), the difference is dramatic.

### Per-Tool Token Savings

How each tool compares to the `rg` equivalent for the same task.
Measured on the booger repo (45 files, 455 chunks).

| Tool | `rg` equiv | booger | Reduction | Notes |
|---|---|---|---|---|
| `references` (count) | 4,446 chars | 32 chars | **139x** | "How many usages of dispatch?" |
| `references` (files_with_matches) | 4,446 chars | 147 chars | **31x** | "Which files use dispatch?" |
| `symbols` (count) | 39,801 chars | 13 chars | **3,317x** | "How many symbols in src/mcp/?" |
| `symbols` (signatures) | 39,801 chars | 9,572 chars | **4x** | Full structural outline |
| `directory-summary` | 55,281 chars | 3,151 chars | **18x** | One-call architectural overview |
| `tests-for` (count) | 713 chars | 12 chars | **59x** | "Are there tests for search?" |
| `changed-since` (count) | 36,150 chars | 13 chars | **3,012x** | "Anything re-indexed today?" |
| `batch` (3x count) | 8,322 chars | 317 chars | **26x** | Search + symbols + refs in one call |
| `grep` (10 results) | 3,241 chars | 905 chars | **4x** | Targeted regex, capped output |

The biggest wins come from count and files_with_matches modes — agents
often just need a number or a file list, not every matching line.
`directory-summary` has no `rg` equivalent at all; you'd need multiple
commands and manual aggregation.

## Performance

Measured on Apple Silicon. 5 projects, 664 files, 4,840 chunks.
All times are averages over 5 runs.

All measurements via MCP (JSON-RPC over stdio), averaged over 5 runs,
on Apple Silicon. Booger project: 45 files, 455 chunks.

### Indexing

| Operation | Time |
|---|---|
| Full index — booger (45 files, 455 chunks) | 65 ms |
| Full index — meld-api (502 files, 2,863 chunks) | 689 ms |
| Full index — fk (78 files, 1,269 chunks) | 144 ms |
| Incremental re-index (no changes) | 23 ms |

### Search & Discovery

| Tool | Avg | Min | Output |
|---|---|---|---|
| `search` (content, 5 results) | 28 ms | 25 ms | 41,775 chars |
| `search` (signatures, 10 results) | 24 ms | 24 ms | 511 chars |
| `search` (count) | 25 ms | 24 ms | 11 chars |
| `grep` (regex) | 25 ms | 25 ms | 905 chars |
| `references` | 27 ms | 26 ms | 4,078 chars |
| `symbols` (signatures, src/mcp/) | 24 ms | 24 ms | 9,572 chars |
| `directory-summary` | 26 ms | 25 ms | 3,151 chars |
| `tests-for` | 27 ms | 26 ms | 20,440 chars |
| `changed-since` (count) | 26 ms | 25 ms | 13 chars |

### Indexing & Status

| Tool | Avg | Min |
|---|---|---|
| `status` | 22 ms | 21 ms |
| `index` (no changes) | 23 ms | 23 ms |

### Git

| Tool | Avg | Min |
|---|---|---|
| `branch-diff` | 59 ms | 58 ms |
| `draft-commit` | 49 ms | 48 ms |

### Multi-tool

| Tool | Avg | Min | Output |
|---|---|---|---|
| `batch` (3 tools) | 33 ms | 33 ms | 317 chars |

### Cross-Project (29 registered projects)

| Tool | Avg | Min | Output |
|---|---|---|---|
| `workspace-search` (signatures, 10) | 256 ms | 186 ms | 1,136 chars |
| `workspace-search` (count) | 187 ms | 183 ms | 109 chars |

### Stress Tests (independently verified by Codex)

| Test | Result |
|---|---|
| 120 calls, 20 parallel clients | p50: 736ms, p95: 1,091ms, 0 failures |
| 60 concurrent workspace-search | p50: 434ms, p95: 1,103ms, 0 failures |

### Sizes

| Metric | Value |
|---|---|
| Binary size | 14 MB |
| Memory (peak RSS, workspace search) | ~14 MB |
