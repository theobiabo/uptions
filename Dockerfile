FROM rust:1-bookworm AS base

WORKDIR /app

COPY . .

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates pkg-config \
    && rm -rf /var/lib/apt/lists/*

FROM base AS dev

RUN apt-get update \
    && apt-get install -y --no-install-recommends libpq-dev \
    && cargo install cargo-watch \
    && rm -rf /var/lib/apt/lists/*

COPY . .

EXPOSE 3000

CMD ["cargo", "watch", "-x", "run"]

FROM base AS builder

COPY . .

RUN cargo build --release --locked --bin uptions-backend

FROM debian:bookworm-slim AS runtime

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/uptions-backend /usr/local/bin/uptions-backend

EXPOSE 3000

CMD ["uptions-backend"]
