# Concepts

A reference for the technologies and ideas behind booger. Each section
explains what it is, why booger uses it, and how it fits in.

---

## MCP (Model Context Protocol)

**What it is:** An open protocol for AI agents to discover and call tools.
It uses [JSON-RPC 2.0](https://www.jsonrpc.org/specification) over stdio
(standard input/output). The agent sends JSON requests, the tool sends
JSON responses — no HTTP server, no sockets, no daemon.

**Why booger uses it:** MCP is the standard that Cursor, Codex, and other
AI coding assistants use to integrate external tools. By implementing MCP,
booger becomes available to any agent without custom integration work.

**How it works in booger:**

```
Agent writes to stdin  →  booger reads line  →  dispatch by method
                                                    ├─ initialize
                                                    ├─ tools/list
                                                    ├─ tools/call → 23 tool handlers
                                                    ├─ resources/list
                                                    └─ resources/read
                                              →  write response to stdout
```

Each request is independent. Booger processes one request, writes one
response, and waits for the next. There is no persistent connection state
beyond what's stored in SQLite.

**Key protocol details:**
- Requests with an `id` field get a response. Requests without `id` are
  notifications (fire-and-forget).
- Error codes follow JSON-RPC 2.0: `-32700` for malformed JSON, `-32600`
  for structurally invalid requests, `-32601` for unknown methods.
- Tool-level errors use `isError: true` in the result payload, not
  protocol-level error codes.

**Links:**
- [MCP Specification](https://modelcontextprotocol.io/)
- [JSON-RPC 2.0 Specification](https://www.jsonrpc.org/specification)

---

## FTS5 (Full-Text Search)

**What it is:** FTS5 is SQLite's built-in full-text search engine. It
creates an inverted index over text content, enabling fast keyword
queries with relevance ranking.

**Why booger uses it:** FTS5 gives us sub-millisecond search over
hundreds of thousands of code chunks without any external service.
It's compiled into SQLite — no extra binary, no network call, no
configuration.

**How it works in booger:**

Booger maintains an FTS5 virtual table (`chunks_fts`) that mirrors the
`chunks` table. SQLite triggers keep them in sync automatically:

```sql
CREATE VIRTUAL TABLE chunks_fts USING fts5(
    name,
    content,
    content='chunks',
    content_rowid='id',
    tokenize='porter unicode61'
);
```

The `porter unicode61` tokenizer applies Porter stemming (so "searching"
matches "search") and handles Unicode text properly.

**Query processing:**
1. User query is sanitized — special FTS5 characters like hyphens and
   dots are quoted so `tree-sitter` doesn't become `tree NOT sitter`.
2. FTS5 returns results ranked by BM25 (a probabilistic relevance model).
3. Booger re-ranks results with code boost, chunk size penalty, and
   volatile context adjustments.
4. If an AND-style query returns no results, booger automatically retries
   with OR between terms.

**Links:**
- [SQLite FTS5 Documentation](https://www.sqlite.org/fts5.html)
- [BM25 (Okapi BM25)](https://en.wikipedia.org/wiki/Okapi_BM25)
- [Porter Stemming Algorithm](https://tartarus.org/martin/PorterStemmer/)

---

## Tree-sitter

**What it is:** A parser generator and incremental parsing library. It
builds concrete syntax trees (CSTs) for source code, giving you a
structured representation of every function, class, import, and
expression in a file.

**Why booger uses it:** Tree-sitter lets booger split source files into
meaningful chunks — individual functions, structs, methods, imports —
instead of arbitrary line ranges or whole files. This means search
results are logical units of code, not random fragments.

**How it works in booger:**

```
source code
  → tree-sitter parse (language-specific grammar)
  → walk AST nodes
  → classify_node() determines chunk kind + name
  → extract_signature() captures params + return type (no body)
  → emit ChunkInsert { kind, name, signature, content, lines, bytes }
```

**Chunking strategy:**
- **Functions/methods**: full body as one chunk, signature extracted separately
- **Containers** (impl, class, trait, interface): signature-only chunk (first
  3 lines), then recurse into children to extract methods individually
- **Imports**: each import/use statement as its own chunk
- **Structs/enums**: declaration as one chunk, signature is the declaration
  line without the field body

**Supported node types per language:**

| Language | Functions | Types | Containers | Imports |
|---|---|---|---|---|
| Rust | `function_item` | `struct_item`, `enum_item`, `type_item` | `impl_item`, `trait_item`, `mod_item` | `use_declaration` |
| Python | `function_definition` | — | `class_definition` | `import_statement`, `import_from_statement` |
| JS/TS | `function_declaration`, arrow functions | `interface_declaration`, `type_alias_declaration`, `enum_declaration` | `class_declaration` | `import_statement`, `require()` |
| Go | `function_declaration`, `method_declaration` | `type_declaration` | — | `import_declaration` |
| C | `function_definition` | `struct_specifier`, `enum_specifier`, `type_definition` | — | `preproc_include` |

**Links:**
- [Tree-sitter](https://tree-sitter.github.io/tree-sitter/)
- [Tree-sitter Playground](https://tree-sitter.github.io/tree-sitter/playground)
- [How Tree-sitter Works (talk)](https://www.youtube.com/watch?v=Jes3bD6P0To)

---

## SQLite

**What it is:** An embedded relational database. Unlike PostgreSQL or
MySQL, SQLite runs inside your process — no server, no TCP, no
configuration. The entire database is a single file.

**Why booger uses it:** SQLite is the ideal storage engine for a local
tool: zero setup, excellent performance, transactional, portable,
and the database file can be trivially backed up or deleted. The
`.booger/index.db` file next to your repo *is* the entire index.

**How booger configures it:**

```sql
PRAGMA journal_mode = WAL;     -- concurrent reads during writes
PRAGMA synchronous = NORMAL;   -- fast writes, safe enough for an index
PRAGMA foreign_keys = ON;      -- cascading deletes (file → chunks → embeddings)
```

**Schema (v5):**

| Table | Purpose |
|---|---|
| `files` | Tracked files with content hash, language, mtime |
| `chunks` | Code chunks with kind, name, signature, content, line/byte ranges |
| `chunks_fts` | FTS5 virtual table, synced via triggers |
| `embeddings` | Vector embeddings as packed f32 BLOBs |
| `annotations` | Volatile notes with optional session scope and TTL |
| `workset` | Focus/visited paths with session scope |
| `meta` | Schema version tracking |

**Links:**
- [SQLite](https://sqlite.org/)
- [WAL Mode](https://www.sqlite.org/wal.html)
- [Why SQLite](https://www.sqlite.org/whentouse.html)

---

## BM25

**What it is:** Okapi BM25 is a probabilistic ranking function used by
search engines to score documents against a query. It considers term
frequency (how often the term appears in a chunk), inverse document
frequency (how rare the term is across all chunks), and document length
normalization.

**Why booger uses it:** FTS5 uses BM25 as its default ranking function.
It gives a solid baseline relevance score that booger then adjusts with
its own re-ranking factors.

**Booger's re-ranking on top of BM25:**

| Factor | Effect | Rationale |
|---|---|---|
| Code boost | +3 for structural chunks | Functions/structs more useful than raw text |
| Chunk size penalty | up to -4 for oversized chunks | Entire READMEs shouldn't dominate results |
| Focus boost | +5 for focused paths | Agent is actively working here |
| Visited penalty | -3 for visited paths | Agent already saw these |
| Annotation boost | +2 for annotated targets | Agent marked these as important |

**Links:**
- [BM25 on Wikipedia](https://en.wikipedia.org/wiki/Okapi_BM25)
- [FTS5 Rank Documentation](https://www.sqlite.org/fts5.html#the_bm25_function)

---

## BLAKE3

**What it is:** A cryptographic hash function that's extremely fast —
significantly faster than SHA-256 or MD5. It produces a 256-bit hash
from arbitrary input.

**Why booger uses it:** Incremental indexing needs to know if a file
has changed since the last index run. Booger hashes every file with
BLAKE3 and compares it to the stored hash. If they match, the file
is skipped entirely. BLAKE3's speed means this check adds negligible
overhead even for large codebases.

**How it works:**

```
file on disk
  → read bytes
  → blake3::hash(bytes)
  → compare with stored content_hash in files table
  → match? skip. different? re-index.
```

**Links:**
- [BLAKE3](https://github.com/BLAKE3-team/BLAKE3)
- [BLAKE3 Paper](https://github.com/BLAKE3-team/BLAKE3-specs/blob/master/blake3.pdf)

---

## Cosine Similarity

**What it is:** A measure of similarity between two vectors. It
computes the cosine of the angle between them — 1.0 means identical
direction, 0.0 means orthogonal (unrelated), -1.0 means opposite.

**Why booger uses it:** Semantic search embeds code chunks and queries
into high-dimensional vectors (768 dimensions with nomic-embed-text).
To find "similar" code, booger computes cosine similarity between the
query vector and every stored embedding, then returns the highest
scoring chunks.

**The formula:**

```
similarity(a, b) = (a · b) / (|a| × |b|)
```

Where `a · b` is the dot product, and `|a|` is the magnitude (L2 norm).

**Performance note:** Booger currently does a brute-force scan over all
embeddings. This is fast enough for typical project sizes (< 100k chunks).
For larger indexes, an approximate nearest neighbor (ANN) index would be
needed.

**Links:**
- [Cosine Similarity on Wikipedia](https://en.wikipedia.org/wiki/Cosine_similarity)
- [Vector Similarity Search Explained](https://www.pinecone.io/learn/vector-similarity/)

---

## Embeddings

**What it is:** An embedding is a dense vector representation of text.
Words, sentences, or code chunks with similar meaning end up close
together in vector space, even if they use completely different words.

**Why booger uses it:** Keyword search (FTS5) finds exact and stemmed
term matches. But "error handling in the database layer" won't match
a function called `recover_from_failure` — they share no words.
Semantic search via embeddings bridges this gap.

**How it works in booger:**

```
code chunk text
  → HTTP POST to Ollama API
  → nomic-embed-text model
  → 768-dimensional f32 vector
  → stored as BLOB in embeddings table
```

**Model details:**
- **nomic-embed-text**: 274 MB model, 768 dimensions, runs locally via Ollama
- Chunks longer than 8192 characters are truncated
- Empty chunks get a placeholder space to avoid API errors
- Embeddings are stored per-chunk and keyed by model name for invalidation

**Hybrid search** combines FTS scores and embedding scores:

```
hybrid_score = alpha × normalized_fts_score + (1 - alpha) × normalized_semantic_score
```

Default alpha is 0.7, favoring keyword matches with semantic backfill.

**Links:**
- [Ollama](https://ollama.ai/)
- [nomic-embed-text](https://ollama.com/library/nomic-embed-text)
- [What Are Embeddings?](https://vickiboykis.com/what_are_embeddings/)

---

## Volatile Context

**What it is:** A session-scoped layer of annotations, focus paths, and
visited paths that influences search ranking without modifying the
permanent index.

**Why booger has it:** AI agents don't just search — they explore. During
a session, an agent builds up knowledge: "I've already looked at config.rs",
"the bug is probably in the parser", "this function is the entry point".
Volatile context lets booger remember these insights and use them to
return better results.

**Three mechanisms:**

### Annotations
Notes attached to a target (file path, symbol name, or `file:line`).
Optional session scoping and TTL for auto-expiry.

```bash
booger annotate src/parser.rs "Known bug in error recovery"
booger annotate dispatch "MCP entry point" --session abc
booger annotate src/store.rs:42 "Off-by-one here" --ttl 3600
```

Annotations appear inline in search results as `[note]` lines and
boost matching results by +2 in ranking.

### Focus
Paths that the agent is actively working on. Search results from
focused paths get a +5 rank boost.

```bash
booger focus src/mcp src/search
```

### Visited
Paths the agent has already reviewed. Search results from visited
paths get a -3 rank penalty, pushing fresh results higher.

```bash
booger visit src/config.rs src/index/walker.rs
```

### Forget
Clears volatile context. Without a session ID, clears everything.
With a session ID, clears only that session's context.

```bash
booger forget                  # clear ALL annotations and workset entries
booger forget --session abc    # clear only session 'abc'
```

**Session scoping** lets multiple agents (or multiple conversations)
use the same index without interfering with each other's context.

---

## Structural Diffing

**What it is:** Comparing code between git revisions at the symbol level
rather than the line level. Instead of "42 lines added, 17 removed",
structural diffing says "function `search` modified, function
`open_if_exists` added, function `old_helper` removed".

**Why booger has it:** Line-level diffs are noisy and hard for agents to
reason about. Structural diffs tell you exactly what changed in terms
the agent already understands — functions, types, imports.

**How it works:**

```
git diff --name-status -z base...HEAD
  → for each changed file:
      git show base:file → tree-sitter parse → base chunks
      read working copy   → tree-sitter parse → head chunks
  → build_chunk_map: key = (kind, name, occurrence_index)
  → compare maps:
      in head but not base → added
      in both, content differs → modified
      in base but not head → removed
```

The `occurrence_index` handles duplicate symbol names (e.g., two
`fn new()` in different `impl` blocks in the same file).

**Consumers:**
- `branch-diff`: returns the full structural diff as JSON
- `draft-commit`: generates a commit message from the diff
- `changelog`: generates grouped Markdown (Added / Modified / Removed)

**Links:**
- [git diff documentation](https://git-scm.com/docs/git-diff)
- [Tree-sitter](https://tree-sitter.github.io/tree-sitter/)

---

## JSON-RPC 2.0

**What it is:** A lightweight remote procedure call protocol encoded
in JSON. Each request has a `method`, `params`, and `id`. Each response
has either a `result` or an `error`.

**Why booger uses it:** MCP is built on JSON-RPC 2.0. Booger implements
the protocol directly over stdio — no HTTP layer, no WebSocket, just
line-delimited JSON.

**Request/response examples:**

```json
// Request
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"search","arguments":{"query":"dispatch"}}}

// Success response
{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"3 result(s)\n..."}]}}

// Error response
{"jsonrpc":"2.0","id":2,"error":{"code":-32601,"message":"Method not found: foo"}}
```

**Notification** (no `id` → no response):
```json
{"jsonrpc":"2.0","method":"notifications/initialized"}
```

**Error codes booger implements:**
| Code | Meaning | When |
|---|---|---|
| `-32700` | Parse error | Malformed JSON (not valid JSON at all) |
| `-32600` | Invalid request | Valid JSON but wrong structure (missing `method`, wrong types) |
| `-32601` | Method not found | Unknown method name with an `id` |
| `-32602` | Invalid params | Missing required parameters (e.g., `resources/read` without `uri`) |

**Links:**
- [JSON-RPC 2.0 Specification](https://www.jsonrpc.org/specification)

---

## Incremental Indexing

**What it is:** Only processing files that have changed since the last
index run. Unchanged files are skipped entirely.

**Why booger does it:** Re-indexing an entire codebase on every search
would be slow. Incremental indexing makes the "auto-index on search"
feature practical — even on large repos, most searches trigger zero
or near-zero re-parsing.

**How it works:**

```
for each file found by directory walk:
  compute BLAKE3 hash of file contents
  look up stored hash in files table
  if hashes match → skip (file unchanged)
  if different or new:
    delete old chunks (CASCADE deletes embeddings too)
    tree-sitter parse → new chunks
    insert into chunks table (triggers update FTS5)
```

Files that existed in the index but no longer exist on disk are
removed automatically.

**Performance:** On a 37-file Rust project (booger itself), a
full re-index takes ~2.5 seconds. An incremental no-change check
takes ~50ms.
