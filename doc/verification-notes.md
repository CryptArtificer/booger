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
