# Oxigit Change Checklists

Use the smallest checklist that fits the change.

## General checklist

- Change is minimal and scoped.
- Existing architecture is preserved.
- No new dependency was added without good reason.
- Naming and module placement follow current repo patterns.

## UI or route change

- Check SSR vs hydrate compatibility.
- Check Leptos route order.
- Ensure shared return types still derive `Serialize`, `Deserialize`, and `Clone` where needed.
- Confirm styling reuses existing patterns.
- Run `make build`.

## Server function change

- Keep server-only imports inside the function body when needed.
- Reuse `server_fns.rs` helpers.
- Check auth and access-control flow.
- If function is used by E2E tests, verify endpoint hash fallout.
- Run `make build`.

## Database change

- Update migrations if schema changed.
- Keep query logic in `oxigit-core`.
- Preserve validation and numbering semantics unless explicitly changing them.
- Check any UI/server serialization impact.
- Run `make build` and relevant tests.

## HTTP git change

- Verify Axum route order.
- Check auth/write-permission handling.
- Check clone/fetch/push behavior.
- Preserve current `git` CLI approach.
- Run `make build`; run `make e2e` if coverage exists.

## SSH change

- Preserve public-key auth flow.
- Preserve path validation and traversal protections.
- Check repository access logic.
- Run `make build`; run `make e2e` if coverage exists.

## E2E-related change

- Rebuild before running tests.
- If a server function was added, renamed, or moved, re-check generated endpoint hashes.
- Update `crates/oxigit-e2e/tests/harness/mod.rs` API constants if needed.
- Run `make e2e`.

## Commands

Common commands:

```bash
make dev
make build
make e2e
cargo test -p oxigit-e2e -- --test-threads=4
make docker
make docker-run
make clean
```
