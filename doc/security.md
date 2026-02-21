# Security

## Threat Model

Booger is a **local, single-user tool** that runs as an MCP server over
stdio. It has no network listener, no HTTP server, and no authentication
layer. The trust boundary is the user's machine — if you can invoke
`booger mcp`, you have full access to its capabilities.

This means:

- **In scope**: preventing accidental data corruption, protecting against
  malformed input from MCP clients, bounding resource usage per request,
  and ensuring no unintended side effects from read-only operations.
- **Not in scope** (local-only tool): authentication, tenant isolation,
  TLS/transport encryption, rate limiting at the transport level, or
  network-facing DoS protection.

If booger gains an HTTP/gRPC server mode in the future, the threat model
will expand to include network-level concerns.

## Verified Properties

These properties were independently verified by Codex through live MCP
probing against the running server. Each item was tested with specific
request/response evidence.

### Input Validation

| Property | How it's enforced |
|---|---|
| SQL injection resistance | All SQL queries use parameterized statements (`?1`, `?2`, ...) via `rusqlite::params!`. No string interpolation into SQL. |
| Command injection resistance | Git commands use `std::process::Command::new` with explicit `.arg()` calls. No shell interpolation, no `sh -c`. |
| FTS5 query sanitization | Special characters (hyphens, dots, slashes, colons, asterisks, carets) in search queries are auto-quoted. `tree-sitter` doesn't become `tree NOT sitter`. |
| Timestamp validation | `changed-since` validates the `since` parameter against RFC 3339 format. Invalid timestamps return an explicit error instead of silent success. |
| Unknown project rejection | Project names that don't match a registered project return `isError: true` with a descriptive message. No silent fallback to a default project. |
| Resource URI exactness | `resources/read` requires an exact URI match against listed resources. `booger://status/fake` is rejected with `-32602`. |

### Resource Limits

| Property | Limit | Rationale |
|---|---|---|
| Batch call count | 20 per request | Prevents CPU amplification via nested batch calls |
| Batch recursion | Blocked | `batch` cannot call `batch` (returns explicit error) |
| Workspace-search threads | 10 concurrent | Caps thread fan-out to prevent resource exhaustion under parallel clients |
| Chunk retrieval | 5x `max_results` fetch, then truncate | Gives re-ranking room to work without unbounded memory |

### Protocol Compliance

| Property | Verified behavior |
|---|---|
| JSON-RPC error taxonomy | Malformed JSON → `-32700` (Parse error). Valid JSON with wrong structure → `-32600` (Invalid Request). Unknown method → `-32601` (Method not found). |
| Notification handling | Requests without an `id` field produce no response (correct per JSON-RPC 2.0). `notifications/initialized` is explicitly handled. |
| MCP serialization safety | `serde_json::to_value()` failures in MCP handlers return `-32603` (Internal error) instead of panicking. No `unwrap()` in MCP response paths. |

### Side-Effect Safety

| Property | How it's enforced |
|---|---|
| Read-only ops don't create files | `Store::open_if_exists` returns `None` if no database exists. `status`, `search`, `annotations`, `forget` never create a `.booger/` directory. |
| Foreign key cascades | Deleting a file automatically deletes its chunks and embeddings. No orphaned data. |

## Concurrency Testing

Codex ran concurrent stress tests against the live MCP server:

| Test | Result |
|---|---|
| 120 calls, 20 parallel clients | p50: 736ms, p95: 1,091ms, 0 failures |
| 80 concurrent workspace-search requests | p50: 465ms, p95: 1,647ms, 0 failures |
| 300-call batch (exceeds limit) | Fails fast with descriptive error |

## Test Coverage

77 unit tests across 4 modules:

| Module | Tests | Scope |
|---|---|---|
| `store/sqlite` | 36 | CRUD, search (all filter combinations), annotations (CRUD, session/target, clear all/scoped), workset, embeddings, transactions, FTS query sanitization, changed-since |
| `mcp/tools` | 28 | Tool dispatch (unknown tool errors), search (missing params, results), batch (limit enforcement, recursion guard, multi-call), timestamp validation, directory-summary, tests-for, symbols, grep (valid + invalid regex), references, status, project resolution, format option parsing |
| `mcp/protocol` | 11 | JSON-RPC success/error response construction, id handling, request/notification parsing, serialization (null field omission), ToolResult/ToolDefinition |
| `config` | 10 | Default config, save/load roundtrip, missing config fallback, storage directory resolution, thread count (auto/explicit), ProjectRegistry (add/remove/resolve) |

## Known Limitations

- **No authentication**: anyone who can invoke the binary has full access.
  Appropriate for local use; not suitable for shared/network deployment.
- **No rate limiting**: a malicious or buggy client can send unlimited
  requests (bounded per-request by batch/thread caps, but not across
  requests).
- **`jsonrpc:"1.0"` accepted**: the server does not enforce `"2.0"` in
  the `jsonrpc` field. Acknowledged as low-risk for MCP clients.
- **Non-spec JSON-RPC id types accepted**: object, array, and float IDs
  are echoed back. Spec only defines string, number, and null.
- **`id:null` ambiguity**: a request with `"id": null` receives a response
  without an `id` field, making correlation ambiguous. Clients should use
  numeric or string IDs.
- **Zero integration tests**: current tests are unit-level. End-to-end
  MCP protocol tests (stdin/stdout round-trips) are manual via Codex
  probing.

## Reporting

If you find a security issue, open an issue or contact the maintainers
directly. This is a local dev tool, not a production service — but we
still want to know.
