/**
 * Seed script: pre-populates a running Oxigit instance with demo data.
 * Run with: npm run seed
 * Requires: Oxigit server running at OXIGIT_URL (default http://127.0.0.1:9100)
 */

import { mkdtempSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { ApiClient } from "../lib/api-client.js";
import {
  cloneRepo,
  createAiCommit,
  pushRepo,
  cleanupDir,
} from "../lib/git-helpers.js";

const BASE_URL = process.env.OXIGIT_URL ?? "http://127.0.0.1:9100";
const USER = "demo";
const EMAIL = "demo@oxigit.dev";
const PASS = "demopass123";
const REPO = "ai-webapp";

async function main() {
  console.log(`Seeding Oxigit at ${BASE_URL}...\n`);

  const api = new ApiClient(BASE_URL);

  // 1. Health check
  console.log("[1/6] Health check...");
  const healthy = await api.healthCheck();
  if (!healthy) {
    console.error(
      `Server not reachable at ${BASE_URL}. Start it with: make dev`,
    );
    process.exit(1);
  }
  console.log("  Server is up.\n");

  // 2. Register
  console.log("[2/6] Registering user...");
  try {
    await api.register(USER, EMAIL, PASS);
  } catch (e: unknown) {
    const msg = e instanceof Error ? e.message : String(e);
    if (msg.includes("409") || msg.includes("already") || msg.includes("UNIQUE")) {
      console.log("  User already exists, skipping.");
    } else {
      throw e;
    }
  }
  console.log();

  // 3. Login
  console.log("[3/6] Logging in...");
  await api.login(USER, PASS);
  console.log();

  // 4. Create repo
  console.log("[4/6] Creating repository...");
  try {
    await api.createRepo(
      REPO,
      "An AI-built web application showcasing Oxigit's AI tracking",
      false,
    );
  } catch (e: unknown) {
    const msg = e instanceof Error ? e.message : String(e);
    if (msg.includes("409") || msg.includes("already") || msg.includes("UNIQUE")) {
      console.log("  Repo already exists, skipping.");
    } else {
      throw e;
    }
  }
  console.log();

  // 5. Clone and create commits with AI metadata (trailers + context.json)
  console.log("[5/6] Creating AI-annotated commits...");
  const tmpDir = mkdtempSync(join(tmpdir(), "oxigit-demo-"));
  const repoDir = join(tmpDir, REPO);

  try {
    cloneRepo(BASE_URL, USER, PASS, USER, REPO, repoDir);

    // ── Session "session-alpha": Claude Code / claude-opus-4-6 ──
    // Prompt 0: "Create the main entry point"
    createAiCommit(
      repoDir,
      "src/main.rs",
      `use std::net::SocketAddr;

mod api;
mod lib;

#[tokio::main]
async fn main() {
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("Server starting on {addr}");
    api::serve(addr).await;
}
`,
      "feat: scaffold main entry point with tokio runtime",
      {
        tool: "claude-code",
        model: "claude-opus-4-6",
        prompt: "Create the main entry point for a Rust web server using tokio",
        session_id: "session-alpha",
        prompt_index: 0,
      },
    );

    // Prompt 1: "Add configuration and domain models"
    createAiCommit(
      repoDir,
      "src/lib.rs",
      `pub mod config {
    pub struct AppConfig {
        pub port: u16,
        pub db_url: String,
        pub log_level: String,
    }

    impl Default for AppConfig {
        fn default() -> Self {
            Self {
                port: 3000,
                db_url: "sqlite://app.db".into(),
                log_level: "info".into(),
            }
        }
    }
}

pub mod models {
    #[derive(Debug, Clone)]
    pub struct User {
        pub id: i64,
        pub username: String,
        pub email: String,
    }

    #[derive(Debug, Clone)]
    pub struct Project {
        pub id: i64,
        pub name: String,
        pub owner_id: i64,
    }
}
`,
      "feat: add config module and domain models",
      {
        tool: "claude-code",
        model: "claude-opus-4-6",
        prompt: "Add configuration and domain model structs",
        session_id: "session-alpha",
        prompt_index: 1,
      },
    );

    // Prompt 2: "Create REST API endpoints" (2 commits from same prompt)
    createAiCommit(
      repoDir,
      "src/api.rs",
      `use std::net::SocketAddr;

pub async fn serve(addr: SocketAddr) {
    // Health endpoint
    let health = || async { "ok" };

    // User endpoints
    let list_users = || async { "[]" };
    let create_user = || async { "created" };

    println!("Listening on http://{addr}");
    // In a real app, mount routes on an axum Router
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn health_returns_ok() {
        let health = || async { "ok" };
        assert_eq!(health().await, "ok");
    }
}
`,
      "feat: implement API endpoint stubs with health check",
      {
        tool: "claude-code",
        model: "claude-opus-4-6",
        prompt: "Create REST API endpoint stubs for users and health check",
        session_id: "session-alpha",
        prompt_index: 2,
      },
    );

    createAiCommit(
      repoDir,
      "src/middleware.rs",
      `pub fn logger(req: &str) -> String {
    let timestamp = "2025-01-01T00:00:00Z"; // placeholder
    format!("[{timestamp}] {req}")
}

pub fn auth_guard(token: &str) -> bool {
    !token.is_empty()
}
`,
      "feat: add request logger and auth guard middleware",
      {
        tool: "claude-code",
        model: "claude-opus-4-6",
        prompt: "Create REST API endpoint stubs for users and health check",
        session_id: "session-alpha",
        prompt_index: 2,
      },
    );

    // ── Session "session-beta": Cursor / gpt-4o ──
    // Prompt 0: "Write integration tests"
    createAiCommit(
      repoDir,
      "src/tests.rs",
      `#[cfg(test)]
mod integration {
    #[tokio::test]
    async fn test_config_defaults() {
        let config = crate::lib::config::AppConfig::default();
        assert_eq!(config.port, 3000);
        assert_eq!(config.db_url, "sqlite://app.db");
    }

    #[tokio::test]
    async fn test_user_model() {
        let user = crate::lib::models::User {
            id: 1,
            username: "alice".into(),
            email: "alice@example.com".into(),
        };
        assert_eq!(user.username, "alice");
    }

    #[tokio::test]
    async fn test_auth_guard() {
        assert!(crate::middleware::auth_guard("valid-token"));
        assert!(!crate::middleware::auth_guard(""));
    }
}
`,
      "test: add integration tests for config, models, and auth",
      {
        tool: "cursor",
        model: "gpt-4o",
        prompt: "Write integration tests for the config, user model, and auth guard",
        session_id: "session-beta",
        prompt_index: 0,
      },
    );

    // Prompt 1: "Set up Cargo.toml"
    createAiCommit(
      repoDir,
      "Cargo.toml",
      `[package]
name = "ai-webapp"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"

[dev-dependencies]
tokio-test = "0.4"
`,
      "chore: configure Cargo.toml with dependencies",
      {
        tool: "cursor",
        model: "gpt-4o",
        prompt: "Set up Cargo.toml with tokio and serde dependencies",
        session_id: "session-beta",
        prompt_index: 1,
      },
    );

    // ── Unsessioned commit: Copilot (no session_id, no prompt_index) ──
    createAiCommit(
      repoDir,
      "README.md",
      `# AI Webapp

A demo Rust web application built with AI assistance.

## Features

- Async HTTP server with tokio
- Configuration management
- User and project domain models
- REST API endpoints
- Request logging and auth middleware

## Getting Started

\`\`\`bash
cargo run
\`\`\`

## Built With

This project was built using multiple AI coding tools:
- **Claude Code** (claude-opus-4-6) — Architecture and core implementation
- **Cursor** (gpt-4o) — Tests and configuration
- **GitHub Copilot** — Documentation
`,
      "docs: add README with project overview",
      {
        tool: "copilot",
        model: "gpt-4o-mini",
        prompt: "Write a README for this Rust web app project",
      },
    );

    // 6. Push
    console.log("\n[6/6] Pushing to Oxigit...");
    pushRepo(repoDir);
  } finally {
    cleanupDir(tmpDir);
  }

  console.log(`\nDone! Visit ${BASE_URL}/${USER}/${REPO} to see the demo data.`);
}

main().catch((err) => {
  console.error("Seed failed:", err);
  process.exit(1);
});
