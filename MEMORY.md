# Oxigit Project Memory

This file contains mutable project memory. Keep it short, factual, and easy to prune.
Move stable guidance into `CLAUDE.md` or `docs/agents/*.md`.

## Current architecture reminders

- `oxigit-app` has two mutually exclusive feature modes: `ssr` and `hydrate`.
- `#[server]` function bodies are server-only.
- Shared server-function return types must remain serializable across client/server boundaries.
- HTTP git routes must be mounted before Leptos routes.
- In the Leptos router, specific routes must appear before catch-all routes.

## Testing reminders

- E2E tests spawn the compiled `oxigit-server` binary.
- Build before E2E runs.
- Renaming or moving a Leptos server function can change its generated endpoint hash and break E2E constants.

## Product reminders

- Oxigit includes AI-related product features such as AI-aware commits, PR summaries, session snapshots, remixing, and deploy previews.
- Keep config and UX changes aligned with existing product patterns rather than introducing parallel mechanisms.

## Maintenance rules for this file

- Remove stale items promptly.
- Do not store speculative notes.
- Prefer concrete pitfalls, recent regressions, or repo-specific lessons.
- If a reminder remains stable for a long time, move it into `CLAUDE.md` or a supporting doc.
