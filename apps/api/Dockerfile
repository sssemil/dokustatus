FROM rust:1.82-slim AS builder

RUN apt-get update \
  && apt-get install -y --no-install-recommends pkg-config libssl-dev ca-certificates build-essential \
  && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Pre-fetch dependencies for faster builds
COPY apps/api/Cargo.toml apps/api/Cargo.lock ./
RUN mkdir -p src && echo "fn main() {}" > src/main.rs
RUN cargo fetch

# Build application
RUN rm -rf src
COPY apps/api/ .
RUN cargo build --release

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates \
  && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/dokustatus /usr/local/bin/dokustatus
COPY apps/api/migrations ./migrations

ENV RUST_LOG=info
EXPOSE 3001

CMD ["dokustatus"]
