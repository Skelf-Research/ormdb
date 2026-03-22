# Multi-stage Dockerfile for ormdb-studio
# Stage 1: Build the Vue.js frontend
FROM node:20-alpine AS frontend-builder

WORKDIR /app/frontend

# Copy frontend source
COPY crates/ormdb-studio/frontend/package*.json ./
RUN npm ci

COPY crates/ormdb-studio/frontend/ ./
RUN npm run build

# Stage 2: Build the Rust binary
FROM rust:1.83-bookworm AS rust-builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the entire workspace
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/

# Copy the built frontend assets
COPY --from=frontend-builder /app/frontend/dist ./crates/ormdb-studio/frontend/dist

# Build the release binary
RUN cargo build --release -p ormdb-studio

# Stage 3: Create minimal runtime image
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 ormdb

WORKDIR /app

# Copy the binary from builder
COPY --from=rust-builder /app/target/release/ormdb-studio /app/ormdb-studio

# Create data directory
RUN mkdir -p /app/data && chown -R ormdb:ormdb /app

USER ormdb

# Expose the default port
EXPOSE 3000

# Environment variables
ENV ORMDB_STUDIO_HOST=0.0.0.0
ENV ORMDB_STUDIO_PORT=3000
ENV ORMDB_STUDIO_DATA_DIR=/app/data

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/ || exit 1

# Run the studio
CMD ["/app/ormdb-studio", "--host", "0.0.0.0", "--port", "3000", "--data-dir", "/app/data", "--no-open"]
