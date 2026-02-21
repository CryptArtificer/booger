# Dependency Licenses

Audited from `Cargo.lock` via `cargo metadata --locked` on 2026-02-21.

> Not legal advice. This is a technical SPDX audit of the resolved
> dependency graph.

## Summary

| Scope | Packages | Unknown | Copyleft/restricted |
|---|---|---|---|
| **Runtime** | 168 | 0 | **0** |
| All (incl. dev/test) | 201 | 0 | 1 (dev-only) |

**Verdict: clean for shipping.** No copyleft or restricted licenses in
the runtime dependency closure.

## License Distribution (all scopes)

| License (SPDX expression) | Count |
|---|---|
| `MIT OR Apache-2.0` | 114 |
| `MIT` | 19 |
| `Unicode-3.0` | 18 |
| `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | 14 |
| `Apache-2.0 OR MIT` | 9 |
| `Apache-2.0` | 6 |
| `BSD-3-Clause` | 5 |
| Other permissive | remaining |

## Flagged Crate

| Crate | Version | License | Scope | Risk |
|---|---|---|---|---|
| `r-efi` | 5.3.0 | `MIT OR Apache-2.0 OR LGPL-2.1-or-later` | dev-only | **None** |

### Why `r-efi` is not a concern

1. It's a UEFI runtime crate — only relevant on UEFI targets
   (Windows EFI boot environments). Does not resolve on macOS or Linux.
2. Dependency path: `r-efi` → `getrandom` → `tempfile` → `booger [dev-dependencies]`
3. Not in the runtime closure at all. Only pulled during `cargo test`.
4. The LGPL is one option in a triple-licensed SPDX expression —
   you can choose MIT or Apache-2.0 instead.

## Direct Dependencies

From `Cargo.toml`:

| Crate | Purpose | License |
|---|---|---|
| `anyhow` | Error handling | MIT OR Apache-2.0 |
| `blake3` | Content hashing (incremental indexing) | MIT OR Apache-2.0 |
| `chrono` | Timestamp handling | MIT OR Apache-2.0 |
| `clap` | CLI argument parsing | MIT OR Apache-2.0 |
| `dirs` | Home directory resolution | MIT OR Apache-2.0 |
| `ignore` | .gitignore-aware directory walking | MIT |
| `regex` | Pattern matching (references, grep) | MIT OR Apache-2.0 |
| `rusqlite` | SQLite database access | MIT |
| `serde` | Serialization/deserialization | MIT OR Apache-2.0 |
| `serde_json` | JSON handling | MIT OR Apache-2.0 |
| `toml` | Config file parsing | MIT OR Apache-2.0 |
| `tree-sitter` | Code parsing engine | MIT |
| `tree-sitter-c` | C grammar | MIT |
| `tree-sitter-go` | Go grammar | MIT |
| `tree-sitter-javascript` | JavaScript grammar | MIT |
| `tree-sitter-python` | Python grammar | MIT |
| `tree-sitter-rust` | Rust grammar | MIT |
| `tree-sitter-typescript` | TypeScript grammar | MIT |
| `ureq` | HTTP client (Ollama API) | MIT OR Apache-2.0 |

Dev dependencies:

| Crate | Purpose | License |
|---|---|---|
| `tempfile` | Temp dirs for tests | MIT OR Apache-2.0 |

## How This Was Audited

1. `cargo metadata --format-version 1 --locked` to resolve the full
   dependency graph from `Cargo.lock`.
2. SPDX license fields extracted for all 201 reachable packages.
3. Runtime vs dev scope separated using `cargo tree --edges normal`.
4. Flagged any package with copyleft markers (GPL, LGPL, AGPL, MPL,
   EUPL, SSPL, OSL, CPAL, EPL, CDDL) in their SPDX expression.
5. Traced each flagged package to determine reachability path and scope.
6. Independently verified by Codex.
