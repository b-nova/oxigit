# Contributing to Oxigit

Thank you for your interest in contributing to Oxigit! This guide covers everything you need to get started.

## Prerequisites

- **Rust stable** (managed via `rust-toolchain.toml`)
- **wasm32-unknown-unknown** target (included in `rust-toolchain.toml`)
- **cargo-leptos** (`cargo install cargo-leptos`)
- **git** (used at runtime for git transport)
- **Docker** (optional, for container builds)

## Getting Started

Clone the repository and build:

```bash
git clone https://github.com/b-nova-openhub/oxigit.git
cd oxigit
make build
```

## Development

Start the dev server with hot reload:

```bash
make dev
# or: cargo leptos watch
```

The server runs at `http://127.0.0.1:9100` by default.

## Building

```bash
make build
# or: cargo leptos build --release
```

This compiles both the server binary and the WASM client bundle.

## Testing

Run unit and integration tests:

```bash
cargo test --workspace
```

Run end-to-end tests (builds the server binary first):

```bash
make e2e
```

E2E tests spawn the compiled `oxigit-server` binary, so the project must be built before running them.

## Code Style

Format your code and check for lints before submitting:

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
```

## Project Structure

Oxigit is a Rust workspace with five crates:

| Crate | Purpose |
|---|---|
| `oxigit-core` | Domain logic, database, auth, git operations |
| `oxigit-app` | Leptos UI, routes, components, server functions |
| `oxigit-server` | Axum server binary, config, route mounting |
| `oxigit-ssh` | SSH server for git transport |
| `oxigit-e2e` | End-to-end tests |

For detailed architecture documentation, see [`docs/agents/architecture.md`](docs/agents/architecture.md).

## Pull Request Guidelines

- Keep changes focused and minimal.
- Add or update tests for your changes.
- Describe **what** changed and **why** in the PR description.
- Ensure `cargo fmt`, `cargo clippy`, and `cargo test --workspace` pass.
- One logical change per PR is preferred over large multi-topic PRs.

## Docker

Build and run locally with Docker:

```bash
make docker
make docker-run
```

## License

By contributing to Oxigit, you agree that your contributions will be licensed under the project's BSL 1.1 license (converting to AGPLv3 on 2029-04-02).
