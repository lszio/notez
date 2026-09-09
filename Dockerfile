# --- builder -------------------------------------------------------------
# Builds the notez web server (package `web`, bin `web`): the
# server-rendered workspace UI plus the protocol API and MCP host.
FROM rust:1-bookworm AS builder
WORKDIR /build

# evil-janet's build script runs bindgen, which needs libclang at build
# time (the Janet C API headers are bound at compile time).
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      clang libclang-dev pkg-config \
 && rm -rf /var/lib/apt/lists/*

# Pre-copy manifests so dependency layers cache before source layers.
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY packages ./packages

# The workspace resolves all members (core/protocol/preview/composition/…),
# but only the `web` server binary is compiled.
RUN cargo build --release -p web --bin web

# --- runtime -------------------------------------------------------------
FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/web /usr/local/bin/notez-web
# The Dioxus SSR renderer reads its template from <bin dir>/public.
COPY --from=builder /build/packages/web/public /usr/local/bin/public

# The server binds via IP/PORT.
# NOTEZ_SPACE_ROOT optionally pins a default space root; the web picker can
# still register/choose spaces at runtime.
ENV IP=0.0.0.0
ENV PORT=3030
# ENV NOTEZ_SPACE_ROOT=/data/space

WORKDIR /data
VOLUME ["/data"]
EXPOSE 3030

ENTRYPOINT ["/usr/local/bin/notez-web"]
