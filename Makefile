.PHONY: dev build clean docker docker-run e2e

dev:
	. "$$HOME/.cargo/env" && cargo leptos watch

build:
	. "$$HOME/.cargo/env" && cargo leptos build --release

clean:
	cargo clean

docker:
	docker build -t oxigit .

docker-run:
	docker compose up -d

e2e: build
	cargo test -p oxigit-e2e -- --test-threads=4

