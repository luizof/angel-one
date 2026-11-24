# syntax=docker/dockerfile:1.7
# Unraid x86_64: Rust musl; OpenSSL (Web Push) usa libssl3 do Alpine.
FROM --platform=$BUILDPLATFORM node:24-alpine AS frontend
WORKDIR /src/web
COPY web/package.json web/package-lock.json ./
RUN --mount=type=cache,target=/root/.npm npm ci --no-audit --no-fund
COPY web/ ./
RUN npm run check && npm run build

FROM --platform=linux/amd64 rust:1.99-alpine AS backend
RUN apk add --no-cache build-base cmake perl openssl-dev
ENV CARGO_TARGET_DIR=/src/target
WORKDIR /src
COPY routing/ ./routing/
COPY server/ ./server/
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked --manifest-path server/Cargo.toml && \
    cp target/release/angel-one /usr/local/bin/angel-one

FROM --platform=linux/amd64 alpine:3.22
RUN apk add --no-cache ca-certificates libssl3 && \
    addgroup -S -g 10001 angel && adduser -S -D -H -u 10001 -G angel angel && \
    mkdir -p /data /srv/web && chown angel:angel /data
COPY --from=backend /usr/local/bin/angel-one /usr/local/bin/angel-one
COPY --from=frontend /src/web/dist/ /srv/web/
ENV PORT=8080 DATA_DIR=/data WEB_DIR=/srv/web TZ_NAME=America/Sao_Paulo
USER 10001:10001
WORKDIR /srv
VOLUME /data
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD wget -q -O /dev/null http://127.0.0.1:8080/api/health || exit 1
ENTRYPOINT ["/usr/local/bin/angel-one"]
