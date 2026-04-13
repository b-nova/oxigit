-include .env
export

.DEFAULT_GOAL := help

# ─── Help ────────────────────────────────────────────────────────────
.PHONY: help
help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-20s\033[0m %s\n", $$1, $$2}'

# ─── Dev ─────────────────────────────────────────────────────────────
.PHONY: dev
dev: ## Start dev server
	. "$$HOME/.cargo/env" && cargo leptos watch

.PHONY: dev-saas
dev-saas: ## Start dev server with SaaS features
	. "$$HOME/.cargo/env" && cargo leptos watch --features saas

# ─── Docker ──────────────────────────────────────────────────────────
.PHONY: docker
docker: ## Build Docker image
	docker build -t oxigit .

# ─── E2E ─────────────────────────────────────────────────────────────
.PHONY: e2e
e2e: ## Run e2e tests
	. "$$HOME/.cargo/env" && cargo leptos build --release
	cargo test -p oxigit-e2e -- --test-threads=4

# ─── Clean ───────────────────────────────────────────────────────────
.PHONY: clean
clean: ## Remove build artifacts
	cargo clean
