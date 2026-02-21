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
