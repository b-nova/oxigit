# Oxigit - Development Guide

## Project Overview
Oxigit is a self-hosted Git hosting platform (GitHub alternative) built in Rust.
Workspace with 5 crates: oxigit-core, oxigit-app, oxigit-server, oxigit-ssh, oxigit-e2e.

## Build & Run Commands
- `make dev` — Dev server with hot reload (`cargo leptos watch`)
- `make build` — Production build (`cargo leptos build --release`)
- `make e2e` — Build + run E2E tests (4 threads)
- `cargo test -p oxigit-e2e -- --test-threads=4` — Run E2E tests only (requires prior build)
- `make docker` / `make docker-run` — Docker build and compose up
- `make clean` — `cargo clean`

## Architecture

### Crate Dependency Graph
```
oxigit-server (binary)
  ├── oxigit-app [features = ["ssr"]]  (Leptos components + server functions)
  ├── oxigit-core                       (models, DB, git ops, auth)
  └── oxigit-ssh                        (SSH server via russh)

oxigit-app (lib: cdylib + rlib)
  └── oxigit-core (optional, only with "ssr" feature)

oxigit-e2e (test-only, spawns compiled binary)
```

### Feature Flags (Critical)
oxigit-app has two mutually exclusive feature sets:
- `ssr` — Server-side rendering. Enables leptos_axum, axum, oxigit-core, database access.
- `hydrate` — Client-side WASM hydration. Enables wasm-bindgen.

Code guarded by `#[cfg(feature = "ssr")]` runs only on the server. The `server_fns` module
and all `#[server]` function bodies are SSR-only. Component markup in pages/ compiles for both targets.

## Leptos 0.8 Patterns

### Data Fetching
```rust
let resource = Resource::new(
    move || dependency_signal.get(),
    move |dep| async move { server_fn(dep).await }
);
view! {
    <Suspense fallback=|| view! { <p>"Loading..."</p> }>
        {move || Suspend::new(async move {
            match resource.await {
                Ok(data) => view! { /* render */ }.into_any(),
                Err(e) => view! { <p>{e.to_string()}</p> }.into_any(),
            }
        })}
    </Suspense>
}
```

### Mutations
```rust
let action = ServerAction::<MyServerFn>::new();
let error = move || action.value().get().and_then(|r| r.err());
view! {
    {move || error().map(|e| view! { <div class="flash flash-error">{e.to_string()}</div> })}
    <ActionForm action=action>
        <input name="field1" />
        <button type="submit">"Submit"</button>
    </ActionForm>
}
```

### Shell/App Split
- `Shell` renders the full HTML document (SSR only, never hydrated).
- `App` is the hydrated component containing Router + Navbar + Routes.

### Route Parameters
```rust
let params = use_params_map();
let owner = move || params.read().get("owner").unwrap_or_default();
```

## Server Function Conventions

All server functions follow this pattern:
```rust
#[server]
async fn my_function(args...) -> Result<ReturnType, ServerFnError> {
    use crate::server_fns::{extract_session_user, get_pool, get_data_dir};
    use oxigit_core::db;
    // 1. Extract pool/data_dir/user
    // 2. Check auth/access
    // 3. Call oxigit_core functions
    // 4. Optionally redirect via leptos_axum::redirect("/path")
}
```

**WASM Gotcha**: Imports of server-only crates (oxigit-core, sqlx, etc.) MUST be inside
the function body, not at module top level. The `#[server]` macro strips the function body
during WASM compilation — top-level imports of server crates will fail WASM builds.

### State & Auth Helpers (server_fns.rs)
- `get_pool()` — Extract SQLite pool from Axum extensions
- `get_data_dir()` — Extract data directory path
- `extract_session_user()` — Parse HMAC-signed session cookie, returns `Option<(i64, String)>`
- `set_session_user(user_id, username)` — Set signed session cookie (7-day, HttpOnly, SameSite=Lax)
- `clear_session()` — Remove session cookie
- `get_base_url()` — Extract from Host header (respects X-Forwarded-Proto)

Session cookie format: `oxigit_session` = `{user_id}:{username}:{hmac_sha256_hex}`

Passwords hashed with Argon2 (default params) via `oxigit_core::auth`.

## Database
- SQLite via sqlx. Pool with `?mode=rwc` and max 5 connections.
- Migrations in `/migrations/` (001-008). Run at startup via `sqlx::migrate!("../../migrations")` (path relative to oxigit-core's Cargo.toml).
- All queries use `sqlx::query_as` with `RETURNING *` for inserts.
- Tables: users, repositories, ssh_keys, collaborators, pull_requests, issues, issue_comments.
- Issues and PRs use per-repo sequential `number` via `COALESCE(MAX(number), 0) + 1`.
- All timestamps: `datetime('now')`. Foreign keys with `ON DELETE CASCADE`.

## Git Operations
- All git ops shell out to the `git` binary via `std::process::Command` (NOT libgit2/gitoxide).
- Bare repos stored at `{data_dir}/repos/{owner}/{repo_name}.git`.
- `repo_path()` in `oxigit_core::git` computes the on-disk path.
- Git env pattern: `Command::new("git").env("GIT_DIR", repo_path)`.
- Merge for PRs: fast-forward via `update-ref`, or three-way merge via git plumbing (read-tree, write-tree, commit-tree, update-ref).

## HTTP Git Smart Protocol (git_http.rs)
Routes in oxigit-server:
- `GET /{owner}/{repo}/info/refs?service=...` — ref advertisement
- `POST /{owner}/{repo}/git-upload-pack` — fetch/clone
- `POST /{owner}/{repo}/git-receive-pack` — push (requires HTTP Basic Auth)

These routes are mounted BEFORE Leptos routes in main.rs (order matters — Axum matches first).
Public repos allow unauthenticated clone. Push always requires auth + write permission.

## SSH Server (oxigit-ssh)
- russh-based, runs on separate port (default 2222), spawned as tokio background task.
- Auth via public key fingerprint lookup in ssh_keys table.
- Parses git commands: `git-upload-pack '/owner/repo.git'` and `git-receive-pack '/owner/repo.git'`.
- Path traversal prevention via username/repo name validation on parsed paths.
- Host key: Ed25519, stored at `{data_dir}/ssh_host_ed25519_key` (auto-generated if missing).

## Shared Response Types
Types used in server function return values (UserInfo, RepoInfo, TreeEntryInfo, CommitSummary,
RepoTreeResponse, etc.) are defined in `oxigit-app/src/pages/mod.rs`. They must derive
`Serialize + Deserialize + Clone` to work across both WASM and server targets.

## Configuration
Clap-derived `Config` struct in `oxigit-server/src/config.rs`. All via env vars or CLI flags:
- `OXIGIT_DATA_DIR` — Root for DB + repos (default: `./data`)
- `OXIGIT_HTTP_ADDR` — HTTP listen address (default: `127.0.0.1:9100`)
- `OXIGIT_SSH_ADDR` — SSH listen address (default: `127.0.0.1:2222`)
- `OXIGIT_SECRET_KEY` — Hex-encoded session key (optional, auto-generated to `data_dir/secret_key`)

## Validation Rules
- Username: 1-39 chars, alphanumeric + hyphens + underscores, no leading/trailing hyphens
- Repo name: 1-100 chars, alphanumeric + hyphens + underscores + dots, no ".." or "." alone
- Password: minimum 8 characters

## CSS & Styling
- Single file: `style/main.css`. Dark theme with CSS variables (`--bg`, `--bg-secondary`, `--border`, `--text`, `--text-secondary`, `--accent`, `--success`, `--danger`).
- Key classes: `.container` (960px), `.card`, `.btn`, `.btn-primary`, `.btn-danger`, `.flash`, `.flash-error`, `.flash-success`, `.repo-list`, `.repo-item`, `.auth-container`, `.form-group`, `.page-header`, `.diff-add`, `.diff-del`.
- No CSS framework. Components also use inline styles.

## Route Ordering Gotcha
In the App component, catch-all routes (`/:user`, `/:owner/:repo`) MUST come AFTER specific
routes (`/login`, `/repos`, `/settings`, etc.) because Leptos router matches top-to-bottom.
