# DockPit

A modern, futuristic Docker container management tool with a beautiful web UI. Manage containers, images, volumes, networks, and Docker Compose stacks across multiple servers — all from one dashboard.

![License](https://img.shields.io/badge/license-MIT-blue)
![Docker](https://img.shields.io/badge/docker-ready-blue)

## Features

### Multi-Server Management
- **Local & Remote** — Manage Docker on the local host and connect remote servers via the DockPit Agent
- **Environment Switching** — Quickly switch between servers from the topbar dropdown
- **Real-time Status** — Live status indicators for all connected servers

### Container Management
- **Full Lifecycle** — Start, stop, restart, recreate, and remove containers
- **Bulk Actions** — Select multiple containers for batch operations
- **Image Update Detection** — Automatically check if container images have newer versions available
- **Recreate with Pull** — One-click pull latest image + recreate container
- **Web Terminal** — Interactive shell access directly in the browser (xterm.js)
- **Log Viewer** — Real-time log streaming with ANSI color support, auto-scroll, line wrapping, and download

### Docker Compose Stacks
- **Create & Deploy** — Write docker-compose.yml with syntax validation and deploy stacks
- **Multi-File Support** — Manage .env files and additional config files per stack
- **Stack Overview** — See running/stopped services at a glance
- **In-Browser Editor** — Edit compose files with YAML validation

### Image, Volume & Network Management
- **Full CRUD** — View, pull, and delete images; manage volumes and networks
- **Usage Detection** — See which resources are in use and which are unused
- **Bulk Cleanup** — Prune unused images, volumes, and networks with one click
- **Force Delete** — Remove stubborn resources with force-delete option

### Dashboard & Widgets
- **Customizable Home** — Drag-and-drop widget grid powered by GridStack
- **Edit Mode** — Enter edit mode to rearrange, resize, add, or remove widgets, then save
- **Per-User Layouts** — Each user gets their own dashboard layout
- **Built-in Widgets:**
  - Quick Stats — Aggregate container/image/volume counts across all servers
  - Server Cards — Status, resource counts, and health per server
  - Container Health — Monitor unhealthy/restarting containers
  - Stack Status — Overview of all stacks with service counts
  - Disk Usage — Docker storage breakdown per server
  - Uptime Monitor — Longest-running containers
  - Unused Resources — Find resources to clean up
  - Quick Actions — One-click system cleanup and update checks

### Update Monitor
- **Automatic Checks** — Schedule periodic image update checks (6h/12h/24h/48h)
- **Update Reports** — See which containers have outdated images
- **Notifications** — Webhook (Slack, Discord, Teams) and email (SMTP) notifications

### User Management & Security
- **Role-Based Access Control** — Super Admin, Admin, Editor, Viewer roles
- **Two-Factor Authentication** — TOTP-based 2FA with QR code setup
- **JWT Authentication** — Secure token-based auth with automatic session management

### Settings
- **Webhook Integration** — Send update reports to Slack, Discord, Microsoft Teams, or any webhook receiver
- **Email Notifications** — SMTP configuration for update reports via email
- **Docker Registry Login** — Store credentials for private registries (Docker Hub, GHCR, GitLab, etc.)

### Design & UX
- **Futuristic UI** — Glassmorphism effects, gradient accents, glow animations
- **Dark & Light Mode** — Perfectly tuned color palettes for both themes
- **Fully Responsive** — 100% mobile-friendly with bottom-sheet modals and touch-optimized controls
- **Custom Components** — All form elements (dropdowns, checkboxes, buttons) are custom-styled, no browser defaults
- **i18n** — Full English and German translation (450+ strings), switchable via UI

## Tech Stack

| Layer | Technology |
|-------|-----------|
| **Backend** | Rust (Axum framework) |
| **Frontend** | SvelteKit 5 + TypeScript |
| **Styling** | Tailwind CSS 4 + Custom Design System |
| **Database** | SQLite |
| **Terminal** | xterm.js + WebSocket |
| **Widgets** | GridStack.js |
| **Docker** | Docker CLI + Compose v2 plugin |
| **Auth** | JWT + TOTP (2FA) |

## Quick Start

### Docker Compose (Recommended)

```yaml
services:
  dockpit:
    image: amslertec/dockpit:latest
    container_name: dockpit
    restart: unless-stopped
    ports:
      - "5533:5533"
    volumes:
      - dockpit_data:/data
      - /var/run/docker.sock:/var/run/docker.sock:ro
      - /var/docker/container:/stacks
    environment:
      - DOCKPIT_PORT=5533
      - DOCKPIT_JWT_SECRET=your-secret-key-here
      - DOCKPIT_STACKS_DIR=/stacks

volumes:
  dockpit_data:
```

Then open `http://localhost:5533` and create your admin account.

### Remote Server Agent

To manage remote Docker hosts, deploy the DockPit Agent:

```yaml
services:
  dockpit-agent:
    image: amslertec/dockpit-agent:latest
    container_name: dockpit-agent
    restart: unless-stopped
    ports:
      - "5522:5522"
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro
      - /var/docker/container:/stacks
    environment:
      - AGENT_STACKS_DIR=/stacks
```

Then connect the remote server in DockPit under **Environments → Connect Remote Server**.

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `DOCKPIT_PORT` | `5533` | HTTP port |
| `DOCKPIT_HTTPS_PORT` | `5539` | HTTPS port (auto TLS) |
| `DOCKPIT_JWT_SECRET` | — | **Required.** Secret key for JWT tokens |
| `DOCKPIT_DB_PATH` | `/data/dockpit.db` | SQLite database path |
| `DOCKPIT_STACKS_DIR` | `/stacks` | Docker Compose stacks directory |

## Development

### Prerequisites
- Rust 1.75+
- Node.js 20+
- Docker

### Frontend
```bash
cd frontend
npm install
npm run dev
```

### Backend
```bash
cargo run --bin dockpit-server
```

### Build Docker Image
```bash
docker compose build
```

## License

MIT
