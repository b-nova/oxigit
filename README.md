# Oxigit

**The AI-native Git platform for vibecoders.**

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

### Docker Compose (Recommended)

```yaml
# docker-compose.yml
services:
  oxigit:
    image: oxigit
    build: .
    ports:
      - "9100:9100"
      - "2222:2222"
    volumes:
      - oxigit-data:/data
    environment:
      - OXIGIT_DATA_DIR=/data
      - OXIGIT_HTTP_ADDR=0.0.0.0:9100
      - OXIGIT_SSH_ADDR=0.0.0.0:2222

volumes:
  oxigit-data:
```

```bash
docker compose up -d
```

Visit `http://localhost:9100` to register your first user.

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

## License

MIT
