# Security

## Threat Model

Booger is designed for **local, trusted developer environments** — CLI
and MCP over stdio. It is not intended to be an internet-facing
multi-tenant service.

The trust boundary is the user's machine. If you can invoke `booger mcp`,
you have full access to its capabilities.

- **In scope**: preventing data corruption, rejecting malformed input,
  bounding resource usage per request, ensuring no side effects from
  read-only operations.
- **Not in scope** (local-only tool): authentication, tenant isolation,
  TLS, rate limiting at the transport level, network-facing DoS protection.

If booger gains an HTTP/gRPC server mode in the future, the threat model
will expand to include network-level concerns.

## Current Security Posture

### Input Validation

| Property | How it's enforced |
|---|---|
| SQL injection resistance | All queries use parameterized statements (`?1`, `?2`, ...) via `rusqlite::params!`. No string interpolation into SQL. |
| Command injection resistance | Git commands use `std::process::Command::new` with explicit `.arg()` calls. No shell interpolation, no `sh -c`. |
| FTS5 query sanitization | Special characters (hyphens, dots, slashes, colons, asterisks, carets) are auto-quoted. `tree-sitter` doesn't become `tree NOT sitter`. |
| Timestamp validation | `changed-since` validates against RFC 3339. Invalid timestamps return an explicit error instead of silent success. |
| Unknown project rejection | Unregistered project names return `isError: true`. No silent fallback to a default project. |
| Resource URI exactness | `resources/read` requires an exact URI match. `booger://status/fake` is rejected with `-32602`. |

### Resource Limits

| Property | Limit | Rationale |
|---|---|---|
| Batch call count | 20 per request | Prevents CPU amplification |
| Batch recursion | Blocked | `batch` cannot call `batch` |
| Workspace-search threads | 10 concurrent | Caps thread fan-out under parallel clients |
| Chunk retrieval | 5x `max_results`, then truncate | Re-ranking room without unbounded memory |
| Indexing | File-size limits + `.gitignore` rules | Prevents accidental heavy processing |

### Protocol Compliance

| Property | Verified behavior |
|---|---|
| JSON-RPC error taxonomy | Malformed JSON → `-32700`. Invalid structure → `-32600`. Unknown method → `-32601`. |
| Notification handling | No-`id` requests produce no response. `notifications/initialized` is explicitly handled. |
| MCP serialization safety | Serialization failures return `-32603` (Internal error). No `unwrap()` in MCP response paths. |

### Side-Effect Safety

| Property | How it's enforced |
|---|---|
| Read-only ops don't create files | `Store::open_if_exists` returns `None` if no database exists. `status`, `search`, `annotations`, `forget` never create a `.booger/` directory. |
| Foreign key cascades | Deleting a file cascades to its chunks and embeddings. No orphaned data. |

## Security Controls in Practice

- Request validation returns structured errors (`isError` / JSON-RPC
  error codes) for all invalid inputs.
- Recursive batch calls are blocked with a descriptive error.
- Unknown project and resource references fail explicitly.
- Indexing respects `.gitignore`, skips binaries, and uses configurable
  file-size limits.

## Concurrency Testing

Independently verified by Codex through live MCP stress testing:

| Test | Result |
|---|---|
| 120 calls, 20 parallel clients | p50: 736ms, p95: 1,091ms, 0 failures |
| 80 concurrent workspace-search | p50: 465ms, p95: 1,647ms, 0 failures |
| 60 concurrent workspace-search (re-verified) | p50: 434ms, p95: 1,103ms, 0 failures |
| 300-call batch (exceeds limit) | Fails fast: "Batch limited to 20 calls, got 300" |

## Test Coverage

77 unit tests across 4 modules:

| Module | Tests | Scope |
|---|---|---|
| `store/sqlite` | 36 | CRUD, search (all filter combinations), annotations (CRUD, session/target, clear all/scoped), workset, embeddings, transactions, FTS query sanitization, changed-since |
| `mcp/tools` | 28 | Tool dispatch, search validation, batch (limit, recursion, multi-call), timestamp validation, directory-summary, tests-for, symbols, grep (valid + invalid regex), references, status, project resolution, format opts |
| `mcp/protocol` | 11 | JSON-RPC response construction, id handling, request/notification parsing, serialization, ToolResult/ToolDefinition |
| `config` | 10 | Save/load roundtrip, defaults, storage dir, thread count, ProjectRegistry CRUD |

## Known Limitations

- **No authentication** — anyone who can invoke the binary has full access.
- **No tenant isolation** — all projects share the same process.
- **No rate limiting** — bounded per-request by batch/thread caps, but
  not across requests.
- **Local resource abuse** — a fully trusted but misbehaving client can
  still consume CPU/disk within per-request bounds.
- **`jsonrpc:"1.0"` accepted** — server does not enforce `"2.0"`. Low
  risk for MCP clients.
- **Non-spec id types accepted** — object, array, and float IDs are
  echoed back. Spec only defines string, number, and null.
- **`id:null` ambiguity** — response omits `id` field, making correlation
  ambiguous. Use numeric or string IDs.
- **Unit tests only** — end-to-end MCP protocol tests are currently
  manual (Codex probing). No automated integration tests yet.

## If You Expose It Beyond Local Use

Security assumptions weaken significantly if wrapped behind a network
service. If you do this:

- Put it behind an authenticated proxy.
- Enforce strict per-request timeouts and concurrency limits.
- Add request size and output size caps.
- Add rate limiting and audit logging.
- Run in a sandboxed runtime with least-privilege filesystem access.

## Security Reporting

If you find a security issue, please report:

- Reproduction steps
- Impact scope
- Environment details
- Minimal proof-of-concept request/response payloads

Open an issue or contact the maintainers directly.
