FROM rust:1.88-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY static ./static
COPY infrastructure ./infrastructure
RUN cargo build --release

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/backend /usr/local/bin/coup-leaderboard
ENV APP_ENV=production
ENV APP_HOST=0.0.0.0
ENV APP_PORT=3000
EXPOSE 3000

CMD ["coup-leaderboard"]
