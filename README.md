# Timelord

An intelligent calendar management platform that syncs your calendars, analyzes scheduling patterns, and suggests data-driven optimizations — all built in Rust.

## Features

- **Multi-provider sync** — imports and incrementally syncs events from Google Calendar and Microsoft Outlook/Exchange
- **Calendar health scoring** — computes a 0–100 score across focus time ratio, meeting fragmentation, RSVP completeness, sync freshness, and optimization adoption
- **Schedule optimization** — uses Integer Linear Programming to suggest event rescheduling; changes are only applied when you explicitly accept them
- **AI agent integration** — exposes a [Model Context Protocol](https://modelcontextprotocol.io) (MCP) server so tools like Claude Code can query and reason about your calendar
- **Multi-tenant** — full org-level data isolation via PostgreSQL Row-Level Security
- **Secure by default** — RS256 asymmetric JWTs, PKCE-required OAuth flows, per-`jti` Redis denylist for sub-minute token revocation

## Architecture

Seven microservices communicate over gRPC (internal) and HTTP (public), with NATS for async domain events:

```
Client → Gateway (:8080)
              ├── Auth (:3001 / gRPC :50051)      – OAuth, JWT, sessions
              └── Calendar (:3002 / gRPC :50052)  – calendars & events

Background services:
  Sync (:3003)       – polls Google / Microsoft, publishes NATS events
  Solver (:3004)     – ILP optimization engine
  Analytics (:3005)  – health score, trends
  MCP (:3006)        – AI agent integration (stdio + HTTP)

Infrastructure:
  PostgreSQL :5433   – primary store (RLS, migrations)
  Redis :6379        – JWT denylist, session cache
  NATS :4222         – domain events (timelord.<entity>.<action>)
```

## Prerequisites

- [Rust stable](https://rustup.rs) — see `rust-toolchain.toml` for the pinned version
- [Docker](https://www.docker.com) — for PostgreSQL, Redis, and NATS
- `protoc` — Protocol Buffers compiler
- `highs` — HiGHS solver (used by the optimizer)

```bash
brew install protobuf highs
```

## Quick Start

```bash
# 1. Copy and configure environment
cp .env.example .env
# Edit .env — add your Google and/or Microsoft OAuth credentials

# 2. Start infrastructure
docker compose up -d

# 3. Build everything
cargo build --workspace

# 4. Run services (each auto-migrates its own schema on startup)
cargo run -p timelord-gateway
cargo run -p timelord-auth
cargo run -p timelord-calendar
```

## Service Ports

| Service    | HTTP  | gRPC  |
|------------|-------|-------|
| Gateway    | 8080  | —     |
| Auth       | 3001  | 50051 |
| Calendar   | 3002  | 50052 |
| Sync       | 3003  | —     |
| Solver     | 3004  | —     |
| Analytics  | 3005  | —     |
| MCP        | 3006  | —     |

## Configuration

Copy `.env.example` to `.env` and fill in the required values. Key settings:

| Variable | Description |
|----------|-------------|
| `DATABASE_URL` | PostgreSQL connection string |
| `REDIS_URL` | Redis connection string |
| `NATS_URL` | NATS server URL |
| `JWT_PRIVATE_KEY_PEM` / `JWT_PUBLIC_KEY_PEM` | RS256 key pair for token signing |
| `TOKEN_ENCRYPTION_KEY` | AES-256-GCM key (32-byte hex) for stored OAuth tokens |
| `GOOGLE_CLIENT_ID` / `GOOGLE_CLIENT_SECRET` | Google OAuth app credentials |
| `MICROSOFT_CLIENT_ID` / `MICROSOFT_CLIENT_SECRET` | Microsoft OAuth app credentials |
| `SYNC_INTERVAL_SECS` | How often the sync service polls providers (default: `300`) |

See `.env.example` for the full list including CORS origins and per-service port overrides.

## Development

```bash
# Unit tests
cargo test --workspace

# Integration tests (requires running PostgreSQL + Redis)
cargo test --workspace -- --ignored

# Lint
cargo clippy --workspace --all-targets

# Format
cargo fmt --all

# Regenerate sqlx offline query cache (run after changing any sqlx::query! macro)
docker compose up -d postgres
DATABASE_URL=postgres://timelord:timelord_dev@localhost:5433/timelord \
  cargo sqlx prepare --workspace
git add .sqlx/
```

## License

MIT
