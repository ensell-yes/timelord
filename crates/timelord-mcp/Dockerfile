FROM rust:1.82-bookworm AS builder
WORKDIR /build
ENV SQLX_OFFLINE=true
RUN apt-get update && apt-get install -y protobuf-compiler && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY crates/timelord-proto/Cargo.toml crates/timelord-proto/Cargo.toml
COPY crates/timelord-common/Cargo.toml crates/timelord-common/Cargo.toml
COPY crates/timelord-auth/Cargo.toml crates/timelord-auth/Cargo.toml
COPY crates/timelord-calendar/Cargo.toml crates/timelord-calendar/Cargo.toml
COPY crates/timelord-gateway/Cargo.toml crates/timelord-gateway/Cargo.toml
COPY crates/timelord-sync/Cargo.toml crates/timelord-sync/Cargo.toml
COPY crates/timelord-solver/Cargo.toml crates/timelord-solver/Cargo.toml
COPY crates/timelord-analytics/Cargo.toml crates/timelord-analytics/Cargo.toml
COPY crates/timelord-mcp/Cargo.toml crates/timelord-mcp/Cargo.toml
RUN for c in timelord-proto timelord-common; do mkdir -p crates/$c/src && echo "" > crates/$c/src/lib.rs; done && for s in timelord-auth timelord-calendar timelord-gateway timelord-sync timelord-solver timelord-analytics timelord-mcp; do mkdir -p crates/$s/src && echo "fn main(){}" > crates/$s/src/main.rs; done
COPY crates/timelord-proto/proto crates/timelord-proto/proto
COPY crates/timelord-proto/build.rs crates/timelord-proto/build.rs
RUN cargo build --release --bin timelord-mcp 2>/dev/null || true
COPY . .
RUN touch crates/timelord-mcp/src/main.rs && cargo build --release --bin timelord-mcp
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/timelord-mcp /usr/local/bin/timelord-mcp
USER 1000:1000
ENTRYPOINT ["timelord-mcp"]
