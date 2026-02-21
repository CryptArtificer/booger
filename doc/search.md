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
function the usage lives in.

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
5 real projects (642 files, 4,699 chunks). Tokens estimated at ~4 chars/token.

| Query | `rg` output | `rg` est. tokens | booger output | booger est. tokens | Reduction |
|---|---|---|---|---|---|
| `parse` | 807,353 chars | ~201,838 | 984 chars | ~246 | **820x** |
| `error` | 2,658,455 chars | ~664,613 | 777 chars | ~194 | **3,421x** |
| `config` | 3,731,276 chars | ~932,819 | 1,004 chars | ~251 | **3,716x** |
| `search` | 110,263,111 chars | ~27,565,777 | 1,200 chars | ~300 | **91,885x** |
| `dispatch` | 975,924 chars | ~243,981 | 858 chars | ~214 | **1,137x** |
| `schema` | 291,142 chars | ~72,785 | 1,079 chars | ~269 | **269x** |
| `test` | 39,512,778 chars | ~9,878,194 | 933 chars | ~233 | **42,350x** |

**Average: 20,514x reduction.** Over a typical session with 5–10 searches,
that's the difference between ~1,000 tokens and ~20,000,000 tokens for the
same codebase understanding.

## Performance

Measured on 5 projects (642 files, 4,699 indexed chunks, Apple Silicon):

| Metric | Value |
|---|---|
| Full index (642 files, 5 projects) | ~2 seconds |
| Incremental re-index (no changes) | ~20 ms |
| Workspace search (4,699 chunks, 5 projects, threaded) | ~40 ms |
| Single-project search | ~30 ms |
| Stress test (120 calls, 20 parallel clients) | p50: 736ms, p95: 1,091ms, 0 failures |
| Memory (peak RSS, workspace search) | ~14 MB |
| Binary size | 14 MB |
