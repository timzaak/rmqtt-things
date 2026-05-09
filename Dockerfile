# Chef stage - cargo-chef for dependency caching
FROM rust:1.91-slim AS chef
RUN cargo install cargo-chef
WORKDIR /app

# Planner stage - analyze dependency graph
FROM chef AS planner
COPY backend/Cargo.toml backend/Cargo.lock ./
COPY backend/src ./src
RUN cargo chef prepare --recipe-path recipe.json

# Builder stage - compile dependencies then application
FROM chef AS builder
RUN apt-get update && apt-get install -y \
    libssl-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Build dependencies only (cached unless Cargo.toml/Cargo.lock changes)
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Copy source and build application
COPY backend/Cargo.toml backend/Cargo.lock ./
COPY backend/src ./src
COPY backend/migrations ./migrations
RUN cargo build --release

# Export OpenAPI spec from backend binary
FROM builder AS openapi-export
RUN ./target/release/rmqtt-things --export-openapi api.json

# Frontend build stage
FROM node:20-slim AS frontend-builder
WORKDIR /app/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ ./
COPY --from=openapi-export /app/api.json ./api.json
RUN npx openapi-ts
RUN npm run build

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary from builder stage
COPY --from=builder /app/target/release/rmqtt-things /app/rmqtt-things

# Copy frontend artifacts
COPY --from=frontend-builder /app/frontend/dist /app/web

# Copy configuration file
COPY backend/config.example.toml /app/config.toml

# Create non-root user
RUN useradd -r -s /bin/false rmqtt
RUN chown -R rmqtt:rmqtt /app
USER rmqtt

EXPOSE 8080

CMD ["./rmqtt-things"]
