# Contributing

## Commits

**Reference the work item in commit messages.**

- Prefer **GitHub issue numbers** when there is an issue: e.g. `Fixes #2` or `Closes #1` in the body, or mention the issue in the title: `Add scope filter on references (#1)`.
- If there’s no issue yet, name the **work item** (e.g. feature or task from [PLAN.md](PLAN.md)) in the first line or body so it’s clear what the commit is for.

Example:

```
Scope filter on references (#1)

- references tool accepts optional scope param (definition|call|type|import)
- Filters results to that ref kind when set
```

## Issues and tasks

Work items from the [Agent Wishlist](PLAN.md#agent-wishlist) and milestones are tracked as **GitHub issues** when we’re ready to implement them. Prefer creating an issue for a task before coding, then reference it in branches and commits (e.g. branch `issue/1-scope-filter`, commit body `Fixes #1`).

**Tasks are gospel.** The issue’s description and acceptance criteria are the spec. Implementation is done when the acceptance criteria are met.

**Before considering any task complete:**

1. **Docs** — Update every doc that mentions the feature, counts (e.g. “N tools”), or related behavior. Grep for stale numbers and references.
2. **Tests** — Add or update tests so the new/changed behavior is covered. Fix any tests that break.
3. **Integration** — Run the feature end-to-end (CLI and/or MCP as relevant): real call, real response. Don’t rely on unit tests alone.

Then verify against the issue. For non-trivial changes, add a short note in [doc/verification-notes.md](doc/verification-notes.md) (gotchas, evidence, integration) so future changes don’t regress the same points.
