## Stage 1: Build SvelteKit frontend
FROM docker.io/library/node:22-slim@sha256:d415caac2f1f77b98caaf9415c5f807e14bc8d7bdea62561ea2fef4fbd08a73c AS frontend
WORKDIR /app/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ ./
RUN npm run build

## Stage 2: Build Rust binary
FROM docker.io/library/rust:slim-trixie@sha256:c03ea1587a8e4474ae1a3f4a377cbb35ad53d2eb5c27f0bdf1ca8986025e322f AS builder
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY Cargo.toml ./
COPY src/ ./src/
COPY --from=frontend /app/frontend/build ./frontend/build/
RUN cargo build --release --bin dockpit-server

## Stage 3: Docker CLI + patched Compose (fixes CVEs in Go dependencies)
FROM docker.io/library/golang:1.26-alpine@sha256:f85330846cde1e57ca9ec309382da3b8e6ae3ab943d2739500e08c86393a21b1 AS compose-builder
ARG COMPOSE_VERSION=v5.1.1
RUN apk add --no-cache git curl \
    && git clone --depth 1 --branch ${COMPOSE_VERSION} https://github.com/docker/compose.git /src
WORKDIR /src
ENV GOTOOLCHAIN=auto
RUN go get github.com/moby/buildkit@v0.28.1 \
    && go get google.golang.org/grpc@v1.79.3 \
    && go get go.opentelemetry.io/otel/sdk@v1.43.0 \
    && go get go.opentelemetry.io/otel@v1.43.0 \
    && go get go.opentelemetry.io/otel/trace@v1.43.0 \
    && go get go.opentelemetry.io/otel/metric@v1.43.0 \
    && go get github.com/sigstore/sigstore@v1.10.4 \
    && go mod tidy
RUN CGO_ENABLED=0 go build -trimpath -ldflags="-s -w" -o /usr/local/bin/docker-compose ./cmd

FROM docker.io/library/alpine:3.21@sha256:48b0309ca019d89d40f670aa1bc06e426dc0931948452e8491e3d65087abc07d AS docker-bins
ARG DOCKER_VERSION=29.4.1
RUN apk add --no-cache curl \
    && curl -fsSL "https://download.docker.com/linux/static/stable/x86_64/docker-${DOCKER_VERSION}.tgz" | tar xz --strip-components=1 -C /usr/local/bin docker/docker
COPY --from=compose-builder /usr/local/bin/docker-compose /usr/local/bin/docker-compose

## Stage: Build Trivy from source (v0.69.3 avoids CVE-2026-33634 which affects >=0.69.4)
FROM docker.io/library/golang:1.26-alpine@sha256:f85330846cde1e57ca9ec309382da3b8e6ae3ab943d2739500e08c86393a21b1 AS trivy-builder
ARG TRIVY_SRC_VERSION=v0.69.3
RUN apk add --no-cache git
RUN git clone --depth 1 --branch ${TRIVY_SRC_VERSION} https://github.com/aquasecurity/trivy.git /src
WORKDIR /src
ENV GOTOOLCHAIN=auto
ENV GOEXPERIMENT=jsonv2
RUN go get github.com/docker/cli@v29.4.1+incompatible \
    && go get github.com/moby/buildkit@v0.28.1 \
    && go get github.com/go-jose/go-jose/v4@v4.1.4 \
    && go get github.com/hashicorp/go-getter@v1.8.6 \
    && go get github.com/moby/spdystream@v0.5.1 \
    && go get go.opentelemetry.io/otel/sdk@v1.43.0 \
    && go get go.opentelemetry.io/otel@v1.43.0 \
    && go get go.opentelemetry.io/otel/trace@v1.43.0 \
    && go get go.opentelemetry.io/otel/metric@v1.43.0 \
    && go get google.golang.org/grpc@v1.79.3 \
    && go mod tidy
RUN CGO_ENABLED=0 GOEXPERIMENT=jsonv2 go build -trimpath -ldflags="-s -w -X github.com/aquasecurity/trivy/pkg/version/app.ver=0.69.3" -o /usr/local/bin/trivy ./cmd/trivy

## Stage 4: Runtime
FROM docker.io/library/debian:stable-slim@sha256:8f0c555de6a2f9c2bda1b170b67479d11f7f5e3b66bb4a7a1d8843361c9dd3ff

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

# Trivy (self-built from source, see trivy-builder stage) + pre-download DB
COPY --from=trivy-builder /usr/local/bin/trivy /usr/local/bin/trivy
RUN trivy filesystem --download-db-only --quiet /tmp && rm -rf /tmp/fanal

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
