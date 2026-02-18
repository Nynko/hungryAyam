# ── Stage 1: Build ─────────────────────────────────────────────────
FROM rust:1.89-bookworm AS builder

WORKDIR /build

# Compile-time checked queries use the .sqlx cache (no DB needed at build time)
ENV SQLX_OFFLINE=true

# Copy manifests first for dependency caching
COPY backend/Cargo.toml backend/Cargo.toml
COPY hungry_ayam_derive/Cargo.toml hungry_ayam_derive/Cargo.toml

# Create dummy source files so `cargo build` can cache dependencies
RUN mkdir -p backend/src && echo "fn main() {}" > backend/src/main.rs \
    && mkdir -p hungry_ayam_derive/src && echo "" > hungry_ayam_derive/src/lib.rs

# Build dependencies only (cached layer)
RUN cd backend && cargo build --release 2>/dev/null || true

# Now copy the real source code
COPY hungry_ayam_derive/ hungry_ayam_derive/
COPY backend/ backend/

# Touch main.rs so cargo knows it changed
RUN touch backend/src/main.rs hungry_ayam_derive/src/lib.rs

# Build the actual binary
RUN cd backend && cargo build --release

# ── Stage 2: Runtime ───────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
        libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the compiled binary
# Migrations are embedded at compile time via sqlx::migrate!()
COPY --from=builder /build/backend/target/release/backend /app/backend

ENV BIND_ADDR=0.0.0.0:3000

EXPOSE 3000

CMD ["/app/backend"]