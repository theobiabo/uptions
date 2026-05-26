FROM rust:1-bookworm AS dev

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libpq-dev pkg-config \
    && cargo install cargo-watch \
    && rm -rf /var/lib/apt/lists/*

EXPOSE 3000

CMD ["cargo", "watch", "-x", "run"]
