.PHONY: dev dev-saas build build-saas clean docker docker-run e2e video

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

docker-run:
	docker compose up -d

e2e: build
	cargo test -p oxigit-e2e -- --test-threads=4

video:
	cd demo && npm run video

