# Build Frontend
FROM node:18-alpine AS frontend-builder
WORKDIR /app
COPY web/package*.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

# Build Backend
FROM rust:1.77-alpine AS backend-builder
WORKDIR /app
RUN apk add --no-cache musl-dev gcc
COPY Cargo.toml Cargo.lock ./
COPY crates/zrc-core/ crates/zrc-core/
COPY crates/zrc-proto/ crates/zrc-proto/
COPY crates/zrc-crypto/ crates/zrc-crypto/
COPY crates/zrc-admin-console/ crates/zrc-admin-console/
# ... copy other crates if needed for workspace resolution

WORKDIR /app/crates/zrc-admin-console
# Copy frontend assets to where rust-embed expects them
COPY --from=frontend-builder /app/dist ./web/dist
RUN cargo build --release

# Runtime
FROM alpine:3.19
WORKDIR /app
COPY --from=backend-builder /app/target/release/zrc-admin-console .
ENV PORT=8080
EXPOSE 8080
CMD ["./zrc-admin-console"]
