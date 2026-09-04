# Multi-stage build for thaumiel-server. Build context should be the repo
# root, e.g.: docker build -f docker/Dockerfile -t thaumiel-server .
#
# Builds with the `saml` feature enabled by default -- this Debian-based
# image is exactly the environment where its native dependencies
# (libxml2/xmlsec1) are a trivial `apt-get install`, unlike a bare Windows
# dev machine (see crates/thaumiel-auth's `saml` feature doc comment and
# docs/CONFIGURATION.md). Pass `--build-arg FEATURES=` (empty) to build
# without it.

FROM rust:latest AS builder
WORKDIR /build

# Native dependencies for the `saml` feature's XML-DSig verification
# (samael's "xmlsec" feature, on by default): libclang for bindgen, the rest
# for xmlsec1/libxml2 itself. Skipped entirely if FEATURES= is passed empty.
RUN apt-get update && apt-get install -y --no-install-recommends \
    libclang-dev libxml2-dev libxmlsec1-dev libxslt1-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*

ARG FEATURES=saml

# Cache dependency compilation separately from application code.
COPY Cargo.toml Cargo.lock ./
COPY crates/thaumiel-core/Cargo.toml crates/thaumiel-core/Cargo.toml
COPY crates/thaumiel-config/Cargo.toml crates/thaumiel-config/Cargo.toml
COPY crates/thaumiel-storage/Cargo.toml crates/thaumiel-storage/Cargo.toml
COPY crates/thaumiel-cache/Cargo.toml crates/thaumiel-cache/Cargo.toml
COPY crates/thaumiel-auth/Cargo.toml crates/thaumiel-auth/Cargo.toml
COPY crates/thaumiel-keygen/Cargo.toml crates/thaumiel-keygen/Cargo.toml
COPY crates/thaumiel-server/Cargo.toml crates/thaumiel-server/Cargo.toml

COPY crates/ crates/
COPY migrations/ migrations/

RUN if [ -n "$FEATURES" ]; then \
        cargo build --release -p thaumiel-server --features "$FEATURES"; \
    else \
        cargo build --release -p thaumiel-server; \
    fi

FROM debian:bookworm-slim AS runtime
# libxml2/libxmlsec1 (the runtime shared libraries, not the -dev headers
# needed only at build time) -- harmless to install even if FEATURES= was
# passed empty at build time, so this stage doesn't need its own ARG.
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libxml2 libxmlsec1 libxmlsec1-openssl \
    && rm -rf /var/lib/apt/lists/*
RUN useradd --system --create-home --shell /usr/sbin/nologin thaumiel

WORKDIR /app
COPY --from=builder /build/target/release/thaumiel-server /usr/local/bin/thaumiel-server
COPY config/ config/

RUN mkdir -p /app/data && chown -R thaumiel:thaumiel /app
USER thaumiel

ENV THAUMIEL_ENV=production
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/thaumiel-server"]
