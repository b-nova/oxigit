/**
 * Git helpers for seeding demo data.
 * Mirrors crates/oxigit-e2e/tests/harness/mod.rs patterns.
 */

import { execFileSync, execSync } from "node:child_process";
import { mkdirSync, writeFileSync, rmSync } from "node:fs";
import { join } from "node:path";

const GIT_ENV = {
  GIT_TERMINAL_PROMPT: "0",
  GIT_AUTHOR_NAME: "Demo User",
  GIT_AUTHOR_EMAIL: "demo@oxigit.dev",
  GIT_COMMITTER_NAME: "Demo User",
  GIT_COMMITTER_EMAIL: "demo@oxigit.dev",
};

function git(args: string[], cwd: string): void {
  execFileSync("git", args, {
    cwd,
    env: { ...process.env, ...GIT_ENV },
    stdio: "pipe",
  });
}

export function cloneRepo(
  baseUrl: string,
  user: string,
  pass: string,
  owner: string,
  repo: string,
  dest: string,
): void {
  const url = `${baseUrl.replace("://", `://${user}:${pass}@`)}/${owner}/${repo}.git`;
  execSync(`git clone ${url} ${dest}`, {
    env: { ...process.env, GIT_TERMINAL_PROMPT: "0" },
    stdio: "pipe",
  });
  // Configure user in cloned repo
  git(["config", "user.email", "demo@oxigit.dev"], dest);
  git(["config", "user.name", "Demo User"], dest);
  console.log(`  Cloned ${owner}/${repo}`);
}

export interface AiCommit {
  tool: string;
  model?: string;
  prompt?: string;
  session_id?: string;
  prompt_index?: number;
}

/**
 * Create a commit with both .oxigit/context.json AND Oxigit git trailers.
 * Using both ensures maximum compatibility with Oxigit's AI metadata extraction.
 * The prompt_index trailer enables prompt-level grouping and the prompt detail page.
 */
export function createAiCommit(
  repoDir: string,
  filename: string,
  content: string,
  subject: string,
  ai: AiCommit,
): void {
  // Write the source file
  const filePath = join(repoDir, filename);
  const parentDir = join(filePath, "..");
  mkdirSync(parentDir, { recursive: true });
  writeFileSync(filePath, content);

  // Write .oxigit/context.json
  const oxigitDir = join(repoDir, ".oxigit");
  mkdirSync(oxigitDir, { recursive: true });
  const ctx: Record<string, string | number> = { tool: ai.tool };
  if (ai.model) ctx.model = ai.model;
  if (ai.prompt) ctx.prompt = ai.prompt;
  if (ai.session_id) ctx.session_id = ai.session_id;
  writeFileSync(join(oxigitDir, "context.json"), JSON.stringify(ctx));

  // Build commit message with Oxigit trailers
  let message = `${subject}\n\nOxigit-Tool: ${ai.tool}`;
  if (ai.model) message += `\nOxigit-Model: ${ai.model}`;
  if (ai.session_id) message += `\nOxigit-Session: ${ai.session_id}`;
  if (ai.prompt) message += `\nOxigit-Prompt: ${ai.prompt}`;
  if (ai.prompt_index !== undefined)
    message += `\nOxigit-Prompt-Index: ${ai.prompt_index}`;

  // Stage and commit
  git(["add", filename, ".oxigit/context.json"], repoDir);
  git(["commit", "-m", message], repoDir);
  console.log(`  Committed: ${subject}`);
}

export function pushRepo(repoDir: string): void {
  git(["push", "origin", "main"], repoDir);
  console.log("  Pushed to origin");
}

export function cleanupDir(dir: string): void {
  rmSync(dir, { recursive: true, force: true });
}
