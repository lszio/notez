# --- builder ---
FROM rust:1-bookworm AS builder
WORKDIR /build
# Pre-copy manifests to cache deps before the source layer.
COPY Cargo.toml Cargo.lock ./
COPY app app
COPY core core
COPY cli cli
COPY adapters adapters
RUN cargo build --release -p app --bin notez-web --features app/target-web

# --- runtime ---
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/notez-web /usr/local/bin/notez-web
ENV NOTEZ_BIND=0.0.0.0:3030
EXPOSE 3030
ENTRYPOINT ["/usr/local/bin/notez-web"]
