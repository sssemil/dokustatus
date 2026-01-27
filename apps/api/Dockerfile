FROM archlinux:base AS builder

RUN pacman -Syu --noconfirm && \
    pacman -S --noconfirm --needed curl ca-certificates base-devel pkgconf openssl git

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal
ENV PATH="/root/.cargo/bin:${PATH}"
RUN cargo install sqlx-cli --no-default-features --features postgres

WORKDIR /build

# Copy workspace root and all workspace members
COPY Cargo.toml ./
COPY libs/reauth-types /build/libs/reauth-types
COPY libs/reauth-sdk-rs /build/libs/reauth-sdk-rs
COPY apps/api /build/apps/api

# Pre-fetch dependencies
RUN cargo fetch --manifest-path apps/api/Cargo.toml

# Build application
ENV SQLX_OFFLINE=true
RUN cargo build --release --manifest-path apps/api/Cargo.toml

FROM archlinux:base AS runtime
RUN pacman -Syu --noconfirm && \
    pacman -S --noconfirm --needed ca-certificates openssl
RUN pacman -Scc --noconfirm

WORKDIR /app
COPY --from=builder /build/target/release/reauth-api /usr/local/bin/reauth-api
COPY --from=builder /root/.cargo/bin/sqlx /usr/local/bin/sqlx
COPY apps/api/migrations ./migrations

ENV RUST_LOG=info
EXPOSE 3001

CMD ["reauth-api"]
