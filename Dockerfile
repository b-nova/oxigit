FROM rust:1.94-bookworm AS builder

# Install wasm target
RUN rustup target add wasm32-unknown-unknown
RUN cargo install cargo-leptos

WORKDIR /app
COPY . .

ARG SAAS_BUILD=false
RUN if [ "$SAAS_BUILD" = "true" ]; then \
      cargo leptos build --release --features saas; \
    else \
      cargo leptos build --release; \
    fi

# Runtime image
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    git \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the server binary
COPY --from=builder /app/target/server-release/oxigit-server /app/oxigit-server

# Copy the site assets (WASM, JS, CSS)
COPY --from=builder /app/target/site /app/target/site

# Copy static assets
COPY --from=builder /app/public /app/public
COPY --from=builder /app/style /app/style

# Create data directory
RUN mkdir -p /app/data/repos

ENV OXIGIT_DATA_DIR=/app/data
ENV OXIGIT_HTTP_ADDR=0.0.0.0:9100
ENV OXIGIT_SSH_ADDR=0.0.0.0:2222

EXPOSE 9100 2222

VOLUME /app/data

CMD ["/app/oxigit-server"]
