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
- All tenant-scoped tables carry `org_id` with RLS policies
- Repos always take `org_id: Uuid` as first parameter — enforced by code review
- **Sync service + provider listing:** all DB operations use `db::set_rls_context()` in transactions
- **Calendar/auth HTTP CRUD:** currently relies on app-layer `org_id` parameters as defense-in-depth; RLS session vars (`SET LOCAL app.current_org_id`) are not yet set per-request. This works because the dev DB user is the table owner (bypasses RLS). **Production TODO:** add per-request RLS middleware or `FORCE ROW LEVEL SECURITY` + split DB roles.

### Audit log
- Every mutation calls `timelord_common::audit::insert_audit()` from the service layer
- `audit_log` is append-only — never UPDATE or DELETE from it

### Migration ownership
- Auth service owns migrations 1-6 (runs at startup)
- Calendar service owns migrations 10-12 (runs at startup)
- Sync service runs calendar migrations with `set_ignore_missing(true)`
- Migration timestamp format: `YYYYMMDDNNNNNN_description.sql`

### NATS domain events
- Calendar mutations publish to `timelord.{calendar,event}.{created,deleted}`
- Sync publishes `timelord.event.synced` and `timelord.event.cancelled`
- Subject format: `timelord.<entity>.<action>`

### Analytics (Phase 4)
- Health score (0-100) computed from: focus time ratio, fragmentation, RSVP completeness, sync freshness, optimization adoption
- NATS listener subscribes to `timelord.>` and updates daily snapshots
- `GET /api/v1/analytics/health` — current score + metric breakdown
- `GET /api/v1/analytics/trends?days=30` — daily score history
- Analytics service owns migrations 30-39

### MCP Server (Phase 4)
- Model Context Protocol server for AI agent integration (Claude CLI/Code)
- Transport: stdio (primary), HTTP health endpoint
- Tools: `list_calendars`, `list_events`, `search_events`, `get_optimization_suggestions`
- Uses `rmcp` 1.2 SDK with `tool_router` + `tool_handler` macros
- Auth context via `MCP_ORG_ID` / `MCP_USER_ID` env vars (session auth in future)

### Calendar optimization (Phase 3)
- Solver service exposes `POST /api/v1/optimize` (trigger) + `POST /api/v1/optimize/:run_id/apply` (accept suggestions)
- ILP formulation: 15-minute time slots, assignment + no-overlap constraints, movement cost objective
- Uses `good_lp` + `clarabel` with LP relaxation (TU constraint matrix gives integral solutions without MIP)
- Suggestions stored in `optimization_suggestions` table; events only moved when user applies them
- Solver service owns migrations 20-29

### Provider sync (Phase 2)
- Sync service polls Google Calendar API and Microsoft Graph on an interval (`SYNC_INTERVAL_SECS`, default 300s)
- Lists work items via `list_sync_work_items()` SECURITY DEFINER function (cross-org RLS bypass)
- Each calendar synced under `SET LOCAL app.current_org_id` for RLS
- Token refresh: `SELECT ... FOR UPDATE` in short transaction, never held across HTTP calls
- Google: uses `syncToken` for incremental sync; 410 Gone → full re-sync
- Microsoft: uses `/events/delta` endpoint; `@removed` → set local status=cancelled
- `TokenEncryptor` lives in `timelord-common` (shared across auth, calendar, sync)

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
