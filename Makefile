.PHONY: dev dev-saas build build-saas clean docker docker-run e2e ensure-infra

INFRA_DIR ?= $(HOME)/Development/infrastructure
INFRA_REPO ?= git@github.com:b-nova/infrastructure.git

ensure-infra:
	@if [ ! -d "$(INFRA_DIR)" ]; then \
		echo "📦 Cloning shared infrastructure..."; \
		git clone $(INFRA_REPO) $(INFRA_DIR); \
	fi
	@$(INFRA_DIR)/scripts/ensure-running.sh

dev:
	. "$$HOME/.cargo/env" && cargo leptos watch

dev-saas:
	. "$$HOME/.cargo/env" && cargo leptos watch --features saas

build:
	. "$$HOME/.cargo/env" && cargo leptos build --release

build-saas:
	. "$$HOME/.cargo/env" && cargo leptos build --release --features saas

clean:
	cargo clean

docker:
	docker build -t oxigit .

docker-run: ensure-infra
	docker compose up -d

e2e: build
	cargo test -p oxigit-e2e -- --test-threads=4


