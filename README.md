# Coup Leaderboard

A web app for tracking completed games of Coup. Admins can create players, submit final placements, correct bad submissions, and view an audit log. The app calculates Weng-Lin ratings and shows a leaderboard based on conservative rating score.

## What It Does

- Login with `admin` and `player` roles.
- Admin-only player management.
- Admin-only completed match submission.
- Placement validation for ranked free-for-all games.
- Weng-Lin rating updates using the `skillratings` crate.
- Rating snapshots per match participant.
- Match history and match detail APIs.
- Match void/correction with full deterministic rating replay.
- Leaderboard with ranked/unranked players.
- Audit log for important changes.
- Minimal browser UI (may migrate it to React if i feel like it).

## Repository Layout

```text
backend/
  src/
    api/              HTTP routes, handlers, errors, UI serving
    application/      config, state, auth, services, repositories
    db/               PostgreSQL connection and migration bootstrap
    domain/           models and validation
  infrastructure/
    migrations/       SQLx migrations
  static/             minimal browser UI assets
  Dockerfile
  docker-compose.yml
```

## Requirements

- Rust toolchain
- Docker and Docker Compose for local PostgreSQL
- PostgreSQL client tools are useful for backup and restore checks

## Local Setup

Start PostgreSQL:

```bash
cd backend
docker compose up -d postgres
```

Create a local environment file from the example:

```bash
cp .env.example .env
```

```env
DATABASE_URL=postgres://admin:admin@localhost:5433/foreign_aid
```

Run the backend:

```bash
cargo run
```

Open the app at:

```text
http://localhost:3000
```

Health endpoints:

```bash
curl http://localhost:3000/healthz
curl http://localhost:3000/readyz
```

## Tests and Checks

Run from `backend/`:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

The test suite expects PostgreSQL to be available with the configured.

## Rating Summary

Players start with internal rating `25.0` and uncertainty `25.0 / 3.0`.

Display rating:

```text
round(rating * 40)
```

Leaderboard rank score:

```text
round((rating - 3 * uncertainty) * 40)
```

Match history is the source of truth. Current ratings are derived and can be rebuilt by replaying confirmed matches in deterministic order.
