# Verification notes

Short records of what was checked for specific features, so future changes don’t regress the same gotchas.

---

## Index-first guidance (#2)

**Likely gotchas**

- “Index-first” guidance string not propagated consistently across search / symbols / references.
- Message text changes breaking tests or external expectations.
- Path used in “Run: booger index …” not matching the storage path used for search.

**Evidence (current codebase)**

- Index-first message formatting is centralized in **src/search/text.rs** (IndexFirstKind and `format_index_first_message`, ~lines 151–169). `explain_empty_search` in the same file (~171–193) uses it for search empty results.
- **MCP tool symbols** uses `format_index_first_message` for: no index (open_if_exists None), and for empty list_symbols when path_has_chunks is false (no indexed files / path prefix empty). **src/mcp/tools.rs** ~1140–1170.
- **MCP tool references** uses it for: no index, and when all_chunks is empty. **src/mcp/tools.rs** ~1302–1332.
- **CLI** search uses `explain_empty_search`, so it gets the same message with path. **src/main.rs** (cmd_search).
- Path in the message is the canonicalized project root (same root passed to MCP `server::run` and to `config.storage_dir(&root)`). So “Run: booger index \"<path>\"" matches what the user must pass to index that project.
- Tests assert the reason prefix (`starts_with(...)`) and presence of `"Run: booger index"` instead of exact full-string equality. **src/mcp/tools.rs** (search_empty_*, references_empty_*, symbols_empty_* tests).

**Verification**

- CLI: empty dir → `booger search "x"` prints message with path. MCP: `references` and `symbols` with empty/no-index return JSON content containing the same style message with path.

---

## Search-expand (#3)

**Acceptance (from issue):** New MCP tool returns search results plus symbols for top N result paths in one call.

**Evidence**

- Tool `search-expand` in **src/mcp/tools.rs**: definition ~line 280, handler `tool_search_expand` ~851, call_tool branch ~646. Runs search, dedupes paths, takes first `expand_top` (default 5, max 20), then `store.list_symbols(Some(path), None)` per path. Output: "Search: N result(s) (expanding top K path(s))" then "--- path ---" and symbol lines. Empty search returns same explain_empty / index-first message as search.
- Tests: `search_expand_returns_search_plus_symbols_for_top_paths`, `search_expand_empty_returns_explain_message`. **src/mcp/tools.rs** ~2270, ~2285.

**Integration**

- MCP: indexed project, `search-expand` with query "dispatch", expand_top 2 → JSON content contains "Search:", "expanding top 2", "--- src/mcp/server.rs ---", and symbol lines. Empty index → content contains "Run: booger index" with path.

**Noted risk (same as other tools)**

- search-expand calls `Store::open_if_exists` after `search()` returns. In normal flow the store was already opened by search, so this is redundant; if there were a race or path mismatch between search and the second open, we could return "No index found" despite having had results. Not introduced by search-expand; references/symbols use the same pattern. Worth remembering when touching store-open logic.
