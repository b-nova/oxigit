# Oxigit

[![License: BSL 1.1](https://img.shields.io/badge/License-BSL_1.1-blue.svg)](LICENSE)
[![Docker](https://img.shields.io/badge/Docker-ghcr.io%2Fb--nova%2Foxigit-blue)](https://ghcr.io/b-nova/oxigit)
[![Built with Rust](https://img.shields.io/badge/Built_with-Rust-orange.svg)](https://www.rust-lang.org/)

**The AI-native Git platform for vibecoders.**

> GitHub tracks what you push. Oxigit tracks how your AI builds it.

Oxigit is a self-hosted Git hosting platform that treats AI-assisted development as a first-class concept. Track what your AI builds, review it with smart summaries, browse coding sessions, remix projects, and deploy previews -- all from a single platform built with Rust.

## Features

### AI-Aware Commits
Every commit tracks which AI tool and prompt generated it. Browse your repo history as a conversation timeline instead of a list of diffs.

### Smart Diff Review
Auto-generated plain-English summaries and rule-based risk detection (security, breaking changes, quality) on every commit and PR. Supports OpenAI, Anthropic, and Ollama for LLM-powered summaries.

### Session Snapshots
AI coding sessions are first-class objects. Browse, review the aggregate diff, get AI summaries, and revert entire sessions with one click.

### Remix & Fork 2.0
Repos with a `REMIX.md` become remixable -- one-click remix that forks the project and shows starter prompts. Discover remixable projects in the Explore gallery.

### Deploy Previews
Webhook-based live previews for every push. Built-in templates for Vercel, Netlify, Cloudflare Pages, DigitalOcean, and Fly.io. External services call back with the preview URL.

### Vibe Scores
Every AI session gets a quality score (0-100, A-F grade) based on efficiency, risk density, churn ratio, and focus. Know at a glance whether a session produced clean code or needs review.

### Guardrails
Rule-based push validation that blocks or warns on security issues, quality problems, or policy violations. Enforce standards on AI-generated code before it lands in your repo.

### Recipe Marketplace
Share successful AI coding sessions as replayable recipes. Browse, search, and replay proven AI workflows in your own repos.

### Full Git Platform
Issues, pull requests, collaborators, SSH keys, public/private repos, forking, HTTP and SSH git protocol. Everything you need, self-hosted.

## One-Click Deploy

[![Deploy on Railway](https://railway.com/button.svg)](https://railway.com/new/template?template=https://github.com/b-nova/oxigit)
[![Deploy to Render](https://render.com/images/deploy-to-render-button.svg)](https://render.com/deploy?repo=https://github.com/b-nova/oxigit)
[![Deploy on DigitalOcean](https://www.deploytodo.com/do-btn-blue.svg)](https://cloud.digitalocean.com/apps/new?repo=https://github.com/b-nova/oxigit/tree/main)

**Fly.io** (supports SSH git access):
```bash
fly launch --from https://github.com/b-nova/oxigit
fly volumes create oxigit_data --size 10
fly deploy
```

> **Note:** Railway and Fly.io support both HTTP and SSH git. Render and DigitalOcean support HTTP git only.

## Quick Start

### Docker Run (Fastest)

```bash
docker run -d --name oxigit \
  -p 9100:9100 -p 2222:2222 \
  -v oxigit-data:/app/data \
  ghcr.io/b-nova/oxigit:main
```

Visit `http://localhost:9100` to register your first user.

### Docker Compose

```yaml
# docker-compose.yml
services:
  oxigit:
    image: ghcr.io/b-nova/oxigit:main
    ports:
      - "9100:9100"
      - "2222:2222"
    volumes:
      - oxigit-data:/app/data

volumes:
  oxigit-data:
```

```bash
docker compose up -d
```

### From Source

```bash
# Prerequisites: Rust toolchain + cargo-leptos
cargo install cargo-leptos

# Build
make build

# Run
make dev
```

## Configuration

All settings via environment variables or CLI flags:

| Variable | Default | Description |
|----------|---------|-------------|
| `OXIGIT_DATA_DIR` | `./data` | Directory for database and bare repos |
| `OXIGIT_HTTP_ADDR` | `127.0.0.1:9100` | HTTP listen address |
| `OXIGIT_SSH_ADDR` | `127.0.0.1:2222` | SSH listen address |
| `OXIGIT_SECRET_KEY` | auto-generated | Hex-encoded session signing key |
| `OXIGIT_LLM_PROVIDER` | `none` | LLM provider: `none`, `openai`, `anthropic`, `ollama` |
| `OXIGIT_LLM_API_KEY` | - | API key for cloud LLM providers |
| `OXIGIT_LLM_MODEL` | `gpt-4o-mini` | LLM model name |
| `OXIGIT_LLM_BASE_URL` | - | Custom LLM endpoint (for Ollama or proxies) |

Users can also configure their own LLM settings in **Settings** (per-user, overrides server defaults).

## AI Metadata Convention

AI tools embed metadata as **git trailers** in commit messages:

```
feat: add user authentication with session cookies

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
Oxigit-Tool: claude-code
Oxigit-Model: claude-opus-4-6
Oxigit-Session: 550e8400-e29b-41d4-a716-446655440000
Oxigit-Prompt: Add user authentication with session cookies
```

Oxigit automatically detects these trailers on push and stores the metadata for display in the AI timeline. Install hooks from the repo settings page to enable this for Claude Code, Codex CLI, and Gemini CLI.

## Architecture

- **Rust** -- Leptos (full-stack SSR + WASM hydration) + Axum + SQLite
- **5 crates**: oxigit-core, oxigit-app, oxigit-server, oxigit-ssh, oxigit-e2e
- **Git protocol**: HTTP Smart Protocol + SSH via russh (pure Rust)
- **Database**: SQLite with sqlx (compile-time checked queries)

## Community

- [GitHub Discussions](https://github.com/b-nova/oxigit/discussions) -- questions, ideas, show & tell
- [Issue Tracker](https://github.com/b-nova/oxigit/issues) -- bug reports and feature requests

## License

[BSL 1.1](LICENSE) — free to self-host; converts to Apache 2.0 after 3 years. See [LICENSING.md](LICENSING.md) for details.
