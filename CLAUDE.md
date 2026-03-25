# Timelord — Developer Guide

## Quick Start

```bash
# Prerequisites: Docker, Rust stable (see rust-toolchain.toml), protoc, coinor-cbc
brew install protobuf          # protoc for proto compilation
brew install highs              # HiGHS solver (zero-dep alternative to coin_cbc)

# Copy env and start infrastructure
cp .env.example .env
# Edit .env with your OAuth credentials

docker compose up -d            # PostgreSQL :5433, Redis :6379, NATS :4222

# Build all crates
cargo build --workspace

# Run a service locally (services auto-run migrations on startup)
cargo run -p timelord-auth
cargo run -p timelord-calendar
cargo run -p timelord-gateway
```

## Service Ports

| Service | HTTP | gRPC |
|---------|------|------|
| Gateway (public) | :8080 | — |
| Auth | :3001 | :50051 |
| Calendar | :3002 | :50052 |
| Sync | :3003 | — |
| Solver | :3004 | — |
| Analytics | :3005 | — |
| MCP | :3006 | — |

## Development Commands

```bash
cargo test --workspace                        # unit tests
cargo test --workspace -- --ignored           # integration tests (requires DB+Redis)
cargo clippy --workspace --all-targets        # linting
cargo fmt --all                               # formatting
cargo sqlx prepare --workspace                # regenerate .sqlx offline cache
```

## Architecture

```
Gateway (:8080)  →  Auth (:3001)  +  Calendar (:3002)  +  [future services]
                         ↓                   ↓
                    PostgreSQL          PostgreSQL + NATS
                    Redis (sessions)
```

### Auth strategy
- RS256 asymmetric JWT; auth holds private key, gateway verifies with public key from JWKS endpoint
- `jti` Redis denylist for sub-minute revocation (no per-request gRPC calls on hot path)
- PKCE required for all OAuth flows
- Personal org auto-created on first login; `users.last_active_org_id` tracks default org

### Multi-tenancy
- All tenant-scoped tables carry `org_id`
- Every DB transaction sets `SET LOCAL app.current_org_id = '<org_id>'` for RLS
- Repos always take `org_id: Uuid` as first parameter — enforced by code review

### Audit log
- Every mutation calls `timelord_common::audit::insert_audit()` from the service layer
- `audit_log` is append-only — never UPDATE or DELETE from it

### Migration ownership
- Auth service owns migrations 1-6 (runs at startup)
- Calendar service owns migrations 10-11 (runs at startup)
- `set_ignore_missing(true)` — services tolerate migrations from other services
- Migration timestamp format: `YYYYMMDDNNNNNN_description.sql`

### NATS domain events
- Calendar mutations publish to `timelord.events.{created,updated,deleted}`
- Subject format: `timelord.<entity>.<action>`

## Adding a new service

1. Create `crates/timelord-<name>/` with `Cargo.toml`, `src/main.rs`
2. Add to workspace `members` in root `Cargo.toml`
3. Add `Dockerfile` and entry in `docker-compose.yml`
4. Define gRPC contract in `crates/timelord-proto/proto/<name>.proto`
5. Add health endpoint at `GET /healthz`

## OAuth app setup

Two separate OAuth app registrations required per provider:

| Environment | Google | Microsoft |
|-------------|--------|-----------|
| Dev | GCP project dev | Azure app registration dev |
| Prod | GCP project prod | Azure app registration prod |

Redirect URIs and CORS origins differ per environment. Never share client secrets between environments.

## sqlx offline mode

CI builds with `SQLX_OFFLINE=true` using the checked-in `.sqlx/` cache.
After changing any `sqlx::query!()` macro, regenerate:

```bash
docker compose up -d postgres
DATABASE_URL=postgres://timelord:timelord_dev@localhost:5433/timelord cargo sqlx prepare --workspace
git add .sqlx/
```

## Commit style

Follows [Conventional Commits](https://www.conventionalcommits.org/):
- `feat:` new feature
- `fix:` bug fix
- `chore:` maintenance, deps
- `docs:` documentation only
- `test:` tests only
- `refactor:` no behavior change
