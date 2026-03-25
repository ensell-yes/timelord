FROM rust:1.82-bookworm AS builder

WORKDIR /build

# Install protoc for tonic-build
RUN apt-get update && apt-get install -y protobuf-compiler && rm -rf /var/lib/apt/lists/*

# Cache workspace dependency graph
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

# Stub src files for dep-caching layer
RUN for crate in timelord-proto timelord-common; do \
      mkdir -p crates/$crate/src && echo "" > crates/$crate/src/lib.rs; \
    done && \
    for svc in timelord-auth timelord-calendar timelord-gateway timelord-sync timelord-solver timelord-analytics timelord-mcp; do \
      mkdir -p crates/$svc/src && echo "fn main(){}" > crates/$svc/src/main.rs; \
    done
COPY crates/timelord-proto/proto crates/timelord-proto/proto
COPY crates/timelord-proto/build.rs crates/timelord-proto/build.rs

RUN cargo build --release --bin timelord-auth 2>/dev/null || true

# Full build
COPY . .
RUN touch crates/timelord-auth/src/main.rs
RUN cargo build --release --bin timelord-auth

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/timelord-auth /usr/local/bin/timelord-auth
COPY --from=builder /build/crates/timelord-auth/migrations /migrations

EXPOSE 3001 50051
USER 1000:1000

ENTRYPOINT ["timelord-auth"]
