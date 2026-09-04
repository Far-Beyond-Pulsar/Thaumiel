# Multi-stage build for thaumiel-server. Build context should be the repo
# root, e.g.: docker build -f docker/Dockerfile -t thaumiel-server .

FROM rust:1.79-bookworm AS builder
WORKDIR /build

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

RUN cargo build --release -p thaumiel-server

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
RUN useradd --system --create-home --shell /usr/sbin/nologin thaumiel

WORKDIR /app
COPY --from=builder /build/target/release/thaumiel-server /usr/local/bin/thaumiel-server
COPY config/ config/

RUN mkdir -p /app/data && chown -R thaumiel:thaumiel /app
USER thaumiel

ENV THAUMIEL_ENV=production
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/thaumiel-server"]
