# Oxigit Claude Code Guide

This file is the entry point for Claude Code in the Oxigit repository.
It contains stable, shared project instructions and imports supporting docs.

## Purpose

Use this repository guidance when reading, modifying, or validating code in Oxigit.
Prefer small, correct changes that preserve the current architecture and conventions.

## Imports

@MEMORY.md
@docs/agents/architecture.md
@docs/agents/checklists.md

## Core operating rules

- Make the smallest correct change.
- Prefer existing patterns over new abstractions.
- Do not refactor broadly unless the task requires it.
- Do not add dependencies unless clearly justified.
- Keep server-only code out of WASM-compiled paths.
- Preserve route ordering in both Axum and Leptos.
- Preserve the current git transport approach based on the `git` CLI.
- Reuse existing response types, validation helpers, and DB access patterns where possible.

## Important repo facts

- Oxigit is a Rust workspace with five crates: `oxigit-core`, `oxigit-app`, `oxigit-server`, `oxigit-ssh`, `oxigit-e2e`.
- The app uses Leptos with SSR and client hydration.
- The server uses Axum.
- The database is SQLite via `sqlx`.
- Git transport is implemented over HTTP and SSH.
- End-to-end tests spawn the compiled server binary.

## File roles

- `CLAUDE.md`: stable project instructions
- `MEMORY.md`: mutable project memory and lessons learned
- `docs/agents/*.md`: supporting architecture and validation docs
- `.claude/settings.json`: shared Claude Code settings
- `.claude/agents/*.md`: specialized subagents
- `.claude/commands/*.md`: reusable slash-command prompts

## Editing constraints

- Do not move server-only imports to module scope in shared app code.
- Do not casually rename or move server functions used by E2E tests.
- Do not weaken path validation in SSH or git transport code.
- Do not introduce a CSS framework.
- Do not replace the `git` CLI with another git implementation unless explicitly requested.

## When in doubt

Check the imported architecture and checklist docs first.
