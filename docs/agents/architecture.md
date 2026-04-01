# Oxigit Architecture Guide

## Workspace layout

- `crates/oxigit-core` — domain logic, database access, auth, git operations, integrations
- `crates/oxigit-app` — Leptos UI, routes, components, server functions
- `crates/oxigit-server` — Axum server binary, config, route mounting, startup
- `crates/oxigit-ssh` — SSH server for git transport
- `crates/oxigit-e2e` — end-to-end tests that spawn the compiled server binary

## Dependency shape

```text
oxigit-server
  ├── oxigit-app [ssr]
  ├── oxigit-core
  └── oxigit-ssh

oxigit-app
  └── oxigit-core (only with "ssr")

oxigit-e2e
  └── spawns compiled oxigit-server binary
```

## Important paths

- `Cargo.toml` — workspace members, shared dependencies, Leptos config
- `migrations/` — SQLite migrations
- `style/main.css` — global styling
- `crates/oxigit-server/src/main.rs` — Axum setup and route mounting
- `crates/oxigit-server/src/config.rs` — env/CLI config
- `crates/oxigit-server/src/git_http.rs` — HTTP smart protocol handlers
- `crates/oxigit-app/src/app.rs` — main app shell and router
- `crates/oxigit-app/src/server_fns.rs` — shared server-side helpers
- `crates/oxigit-app/src/pages/` — route components, server functions, shared response types
- `crates/oxigit-core/src/` — DB logic, auth, git logic, domain code
- `crates/oxigit-ssh/src/` — SSH git transport
- `crates/oxigit-e2e/tests/harness/mod.rs` — E2E harness and API constants

## SSR and hydrate rules

`oxigit-app` has two mutually exclusive feature sets:

- `ssr` — server-side rendering, Axum integration, database access, core logic
- `hydrate` — client-side WASM hydration

Critical constraints:

- Code behind `#[cfg(feature = "ssr")]` is server-only.
- `#[server]` function bodies are server-only.
- UI components often compile for both targets.
- Shared return types must remain serializable.

### WASM gotcha

Do not put imports of server-only crates at module top level in files compiled for the hydrate target.
Keep imports for server-only code inside `#[server]` function bodies.

## Server-function conventions

Typical server-function flow:

1. Extract request-scoped state using helpers from `server_fns.rs`
2. Extract or validate the current session user
3. Check authorization and repository access
4. Call into `oxigit-core`
5. Redirect if needed

Shared helpers include:

- `get_pool()`
- `get_data_dir()`
- `extract_session_user()`
- `set_session_user(...)`
- `clear_session()`
- `get_base_url()`

## Database rules

- Database is SQLite via `sqlx`.
- Migrations live in `/migrations/`.
- Migrations are run at startup.
- Keep query logic in `oxigit-core`.
- Preserve per-repo issue/PR numbering unless the task explicitly changes it.

## Git transport rules

- Git operations shell out to the `git` binary through `std::process::Command`.
- Bare repositories live under `{data_dir}/repos/{owner}/{repo}.git`.
- Repository path computation belongs in `oxigit-core::git`.

## HTTP git routing rules

Routes include:

- `GET /{owner}/{repo}/info/refs?service=...`
- `POST /{owner}/{repo}/git-upload-pack`
- `POST /{owner}/{repo}/git-receive-pack`

Critical:

- Mount these routes before Leptos routes in `main.rs`.
- Public repositories may allow unauthenticated clone/fetch.
- Push requires authentication and write permission.

## SSH rules

- SSH server runs on a separate port.
- Authentication is based on public key lookup in `ssh_keys`.
- Keep path validation strict.
- Do not weaken traversal protections.

## Router ordering rules

In Leptos, specific routes must come before catch-all routes.
Examples of catch-all routes that must stay late:

- `/:user`
- `/:owner/:repo`

Examples of specific routes that must stay earlier:

- `/login`
- `/repos`
- `/settings`

## Styling rules

- Global styles are in `style/main.css`.
- Reuse existing classes and conventions.
- Avoid introducing a CSS framework.
