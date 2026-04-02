.PHONY: dev build clean docker docker-run e2e brand-assets

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

brand-assets:
	rsvg-convert public/brand/logo-mark.svg -w 16 -h 16 -o public/favicon-16x16.png
	rsvg-convert public/brand/logo-mark.svg -w 32 -h 32 -o public/favicon-32x32.png
	rsvg-convert public/brand/logo-mark.svg -w 180 -h 180 -o public/apple-touch-icon.png
	rsvg-convert public/og-image.svg -w 1200 -h 630 -o public/og-image.png
