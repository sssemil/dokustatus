FROM archlinux:base AS builder

RUN pacman -Syu --noconfirm && \
    pacman -S --noconfirm --needed curl ca-certificates base-devel pkgconf openssl git

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal
ENV PATH="/root/.cargo/bin:${PATH}"
RUN cargo install sqlx-cli --no-default-features --features postgres

WORKDIR /app

# Pre-fetch dependencies for faster builds
COPY apps/api/Cargo.toml apps/api/Cargo.lock ./
RUN mkdir -p src && echo "fn main() {}" > src/main.rs
RUN cargo fetch

# Build application
RUN rm -rf src
COPY apps/api/ .
ENV SQLX_OFFLINE=true
RUN cargo build --release

FROM archlinux:base AS runtime
RUN pacman -Syu --noconfirm && \
    pacman -S --noconfirm --needed ca-certificates openssl
RUN pacman -Scc --noconfirm

WORKDIR /app
COPY --from=builder /app/target/release/reauth-api /usr/local/bin/reauth-api
COPY --from=builder /root/.cargo/bin/sqlx /usr/local/bin/sqlx
COPY apps/api/migrations ./migrations

ENV RUST_LOG=info
EXPOSE 3001

CMD ["reauth-api"]
