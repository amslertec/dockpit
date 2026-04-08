## Stage 1: Build SvelteKit frontend
FROM docker.io/library/node:20-slim AS frontend
WORKDIR /app/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ ./
RUN npm run build

## Stage 2: Build Rust binary
FROM docker.io/library/rust:slim-trixie AS builder
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY Cargo.toml ./
COPY src/ ./src/
COPY --from=frontend /app/frontend/build ./frontend/build/
RUN cargo build --release --bin dockpit-server

## Stage 3: Docker CLI + patched Compose (fixes CVEs in Go dependencies)
FROM docker.io/library/golang:1.26-alpine AS compose-builder
ARG COMPOSE_VERSION=v5.1.1
RUN apk add --no-cache git curl \
    && git clone --depth 1 --branch ${COMPOSE_VERSION} https://github.com/docker/compose.git /src
WORKDIR /src
ENV GOTOOLCHAIN=auto
RUN go get github.com/docker/docker@v29.3.1 \
    && go get github.com/docker/cli@v29.3.1 \
    && go get github.com/moby/buildkit@v0.28.1 \
    && go get google.golang.org/grpc@v1.79.3 \
    && go get go.opentelemetry.io/otel/sdk@v1.40.0 \
    && go get go.opentelemetry.io/otel@v1.40.0 \
    && go get go.opentelemetry.io/otel/trace@v1.40.0 \
    && go get go.opentelemetry.io/otel/metric@v1.40.0 \
    && go get github.com/sigstore/sigstore@v1.10.4 \
    && go get github.com/go-jose/go-jose/v4@v4.1.4 \
    && go mod tidy
RUN CGO_ENABLED=0 go build -trimpath -ldflags="-s -w" -o /usr/local/bin/docker-compose ./cmd

FROM docker.io/library/alpine:3.21 AS docker-bins
ARG DOCKER_VERSION=29.3.1
RUN apk add --no-cache curl \
    && curl -fsSL "https://download.docker.com/linux/static/stable/x86_64/docker-${DOCKER_VERSION}.tgz" | tar xz --strip-components=1 -C /usr/local/bin docker/docker
COPY --from=compose-builder /usr/local/bin/docker-compose /usr/local/bin/docker-compose

## Stage 4: Runtime
FROM docker.io/library/debian:trixie-slim

LABEL org.opencontainers.image.title="DockPit" \
      org.opencontainers.image.description="Modern Docker container management tool" \
      org.opencontainers.image.vendor="amslertec" \
      org.opencontainers.image.source="https://github.com/amslertec/dockpit" \
      org.opencontainers.image.licenses="MIT"

RUN apt-get update && apt-get upgrade -y && apt-get install -y \
    ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

# Install Docker CLI + Compose Plugin from static binaries (no vulnerable Go deps)
COPY --from=docker-bins /usr/local/bin/docker /usr/local/bin/docker
COPY --from=docker-bins /usr/local/bin/docker-compose /usr/libexec/docker/cli-plugins/docker-compose
RUN mkdir -p /usr/libexec/docker/cli-plugins

# Install Docker Scout CLI plugin
ARG SCOUT_VERSION=v1.20.4
RUN curl -fsSL "https://github.com/docker/scout-cli/releases/download/${SCOUT_VERSION}/docker-scout_${SCOUT_VERSION#v}_linux_amd64.tar.gz" \
    | tar xz -C /usr/libexec/docker/cli-plugins docker-scout

COPY --from=builder /app/target/release/dockpit-server /usr/local/bin/dockpit-server

RUN mkdir -p /data/certs

ENV DOCKPIT_DB_PATH=/data/dockpit.db
ENV DOCKPIT_PORT=5533
ENV DOCKPIT_HTTPS_PORT=5539

EXPOSE 5533
EXPOSE 5539

VOLUME ["/data"]

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -sf http://localhost:5533/api/status || exit 1

CMD ["dockpit-server"]
