#!/bin/bash
set -e

# ============================================================
# DockPit Docker Hub Push Script
# Usage:
#   ./docker-push.sh              → push latest only
#   ./docker-push.sh v0.1.0       → push latest + v0.1.0 tag
#   ./docker-push.sh v0.1.0 --no-agent  → skip agent build
# ============================================================

DOCKPIT_IMAGE="amslertec/dockpit"
AGENT_IMAGE="amslertec/dockpit-agent"
VERSION="${1:-}"
SKIP_AGENT=false

for arg in "$@"; do
    if [ "$arg" = "--no-agent" ]; then
        SKIP_AGENT=true
    fi
done

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

info() { echo -e "${BLUE}[INFO]${NC} $1"; }
ok()   { echo -e "${GREEN}[OK]${NC}   $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
err()  { echo -e "${RED}[ERR]${NC}  $1"; exit 1; }

# Check prerequisites
command -v docker >/dev/null 2>&1 || err "Docker ist nicht installiert"
docker info >/dev/null 2>&1 || err "Docker daemon nicht erreichbar"

# Ensure logged into Docker Hub
DOCKER_USER=$(docker info 2>/dev/null | grep -i "username" | awk '{print $2}')
if [ "$DOCKER_USER" != "amslertec" ]; then
    warn "Nicht als 'amslertec' eingeloggt. Bitte einloggen:"
    docker login -u amslertec || err "Docker Login fehlgeschlagen"
fi

# Ensure buildx builder exists with docker-container driver (needed for attestations)
BUILDER_NAME="dockpit-builder"
if ! docker buildx inspect "$BUILDER_NAME" >/dev/null 2>&1; then
    info "Erstelle buildx Builder für Supply Chain Attestations..."
    docker buildx create --name "$BUILDER_NAME" --driver docker-container --use
else
    docker buildx use "$BUILDER_NAME"
fi

echo ""
echo -e "${BLUE}╔══════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║        DockPit Docker Hub Push           ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════╝${NC}"
echo ""

if [ -n "$VERSION" ] && [ "$VERSION" != "--no-agent" ]; then
    info "Version: ${GREEN}${VERSION}${NC} + latest"
else
    info "Version: ${GREEN}latest${NC}"
fi
echo ""

# ──────────────────────────────────────────
# Build & Push DockPit Server
# ──────────────────────────────────────────
info "Building DockPit Server..."

DOCKPIT_TAGS="-t ${DOCKPIT_IMAGE}:latest"
if [ -n "$VERSION" ] && [ "$VERSION" != "--no-agent" ]; then
    DOCKPIT_TAGS="${DOCKPIT_TAGS} -t ${DOCKPIT_IMAGE}:${VERSION}"
fi

docker buildx build \
    --platform linux/amd64 \
    --file Dockerfile \
    --push \
    --sbom=true \
    --provenance=mode=max \
    --label "org.opencontainers.image.version=${VERSION:-latest}" \
    --label "org.opencontainers.image.created=$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    ${DOCKPIT_TAGS} \
    .

ok "DockPit Server pushed: ${DOCKPIT_IMAGE}:latest"
if [ -n "$VERSION" ] && [ "$VERSION" != "--no-agent" ]; then
    ok "DockPit Server pushed: ${DOCKPIT_IMAGE}:${VERSION}"
fi
echo ""

# ──────────────────────────────────────────
# Build & Push DockPit Agent
# ──────────────────────────────────────────
if [ "$SKIP_AGENT" = true ]; then
    warn "Agent-Build übersprungen (--no-agent)"
else
    info "Building DockPit Agent..."

    docker buildx build \
        --platform linux/amd64 \
        --file Dockerfile.agent \
        --push \
        --sbom=true \
        --provenance=mode=max \
        --label "org.opencontainers.image.version=latest" \
        --label "org.opencontainers.image.created=$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
        -t ${AGENT_IMAGE}:latest \
        .

    ok "DockPit Agent pushed: ${AGENT_IMAGE}:latest"
fi

echo ""
echo -e "${GREEN}╔══════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║            Push abgeschlossen!           ║${NC}"
echo -e "${GREEN}╚══════════════════════════════════════════╝${NC}"
echo ""
echo -e "  Server: ${BLUE}${DOCKPIT_IMAGE}:latest${NC}"
if [ -n "$VERSION" ] && [ "$VERSION" != "--no-agent" ]; then
    echo -e "          ${BLUE}${DOCKPIT_IMAGE}:${VERSION}${NC}"
fi
if [ "$SKIP_AGENT" != true ]; then
    echo -e "  Agent:  ${BLUE}${AGENT_IMAGE}:latest${NC}"
fi
echo ""
