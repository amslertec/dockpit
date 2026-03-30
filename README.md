<p align="center">
  <img src="frontend/static/logo.svg" alt="DockPit Logo" width="80" height="80">
</p>

<h1 align="center">DockPit</h1>

<p align="center">
  <strong>A modern, self-hosted Docker management platform with a futuristic web UI.</strong><br>
  Monitor, manage, and secure your containers across multiple servers — all from one dashboard.
</p>

<p align="center">
  <a href="https://github.com/amslertec/dockpit/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue" alt="License"></a>
  <a href="https://hub.docker.com/r/amslertec/dockpit"><img src="https://img.shields.io/docker/pulls/amslertec/dockpit?color=blue&label=pulls" alt="Docker Pulls"></a>
  <a href="https://hub.docker.com/r/amslertec/dockpit"><img src="https://img.shields.io/docker/image-size/amslertec/dockpit/latest?label=image%20size" alt="Image Size"></a>
  <a href="https://github.com/sponsors/amslertec"><img src="https://img.shields.io/badge/sponsor-GitHub%20Sponsors-ea4aaa" alt="Sponsor"></a>
  <a href="https://www.buymeacoffee.com/amslertec"><img src="https://img.shields.io/badge/Buy%20Me%20a%20Coffee-ffdd00?logo=buy-me-a-coffee&logoColor=black" alt="Buy Me a Coffee"></a>
</p>

---

## What is DockPit?

DockPit is a **self-hosted Docker management platform** built from the ground up with Rust and SvelteKit. It gives you full control over your Docker infrastructure through a sleek, responsive web interface — without sending any data to external services.

**Why DockPit?**

- **Fast** — Rust backend with zero-overhead Docker socket communication
- **Lightweight** — Single binary + SQLite, no external dependencies
- **Multi-Server** — Manage local and remote Docker hosts from one UI via the DockPit Agent
- **Secure** — Role-based access, 2FA, JWT auth, CSRF protection, one-time WebSocket tokens
- **Beautiful** — Glassmorphism design with dark/light mode, fully responsive

---

## Key Features

### Infrastructure Management

| Feature | Description |
|---------|-------------|
| **Containers** | Full lifecycle control — start, stop, restart, recreate, remove, bulk actions |
| **Compose Stacks** | Create, deploy, edit docker-compose.yml with in-browser YAML editor |
| **Images** | Pull, inspect, delete images; detect unused images for cleanup |
| **Volumes & Networks** | View, create, prune unused resources with one click |
| **Web Terminal** | Interactive shell access to any container (xterm.js + WebSocket) |
| **Log Viewer** | Real-time log streaming with ANSI colors, timestamps, and download |

### Monitoring & Observability

| Feature | Description |
|---------|-------------|
| **Live Resource Monitor** | Real-time CPU, RAM, Network I/O per container via WebSocket (2s updates) |
| **Health Check Dashboard** | Docker HEALTHCHECK status, failing streaks, health logs |
| **Container Event Log** | Live timeline of start/stop/restart/OOM events with 7-day retention |
| **Update Detection** | Registry API-based digest comparison — detects outdated images without pulling |
| **Prometheus Metrics** | `/api/metrics` endpoint for Grafana dashboards and alerting |

### Security & Access Control

| Feature | Description |
|---------|-------------|
| **Role-Based Access** | 4 roles — Super Admin, Admin, Editor, Viewer |
| **Two-Factor Auth (2FA)** | TOTP-based with QR code setup |
| **Audit Log** | Every user action logged with timestamps, 30-day retention |
| **Vulnerability Scanner** | Docker Scout CVE scanning with severity breakdown and NVD links |
| **CSRF Protection** | Origin header validation on all state-changing requests |

### Automation & Notifications

| Feature | Description |
|---------|-------------|
| **Scheduled Jobs** | Cron-like automation — update checks, system prune, stack redeploy |
| **Notification Center** | Persistent notifications with per-type preferences |
| **Webhooks** | Slack, Discord, Microsoft Teams integration |
| **Email Alerts** | SMTP support for update reports |

### UX & Customization

| Feature | Description |
|---------|-------------|
| **Customizable Dashboard** | Drag-and-drop widget grid with multiple tabs, colors, import/export |
| **Command Palette** | `Ctrl+K` to search containers, stacks, servers, and pages instantly |
| **Dark & Light Mode** | Glassmorphism design with gradient accents |
| **i18n** | English and German (450+ strings) |
| **Fully Responsive** | Mobile-optimized with touch controls |

---

## Quick Start

### 1. Deploy DockPit

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
      - DOCKPIT_JWT_SECRET=change-me-to-a-secure-random-string
      - DOCKPIT_STACKS_DIR=/stacks

volumes:
  dockpit_data:
```

```bash
docker compose up -d
```

Open **http://localhost:5533** and create your admin account.

### 2. Add Remote Servers (Optional)

Deploy the **DockPit Agent** on any remote Docker host:

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

Then connect in DockPit under **Environments → Connect Remote Server**.

---

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `DOCKPIT_PORT` | `5533` | HTTP port |
| `DOCKPIT_HTTPS_PORT` | `5539` | HTTPS port (auto TLS if certs provided) |
| `DOCKPIT_JWT_SECRET` | — | **Required.** Secret key for JWT tokens (min. 16 characters) |
| `DOCKPIT_DB_PATH` | `/data/dockpit.db` | SQLite database path |
| `DOCKPIT_STACKS_DIR` | `/stacks` | Docker Compose stacks directory |

---

## Prometheus Integration

Add DockPit as a scrape target in your `prometheus.yml`:

```yaml
scrape_configs:
  - job_name: 'dockpit'
    scrape_interval: 30s
    static_configs:
      - targets: ['your-server:5533']
    metrics_path: '/api/metrics'
```

**Available metrics:** `dockpit_containers_total`, `dockpit_images_total`, `dockpit_volumes_total`, `dockpit_networks_total`, `dockpit_health_status`, `dockpit_updates_outdated`, `dockpit_stacks_total`, `dockpit_environments_total`, `dockpit_users_total`, `dockpit_notifications_unread`, `dockpit_scheduled_jobs_total`

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| **Backend** | Rust + Axum |
| **Frontend** | SvelteKit 5 + TypeScript |
| **Styling** | Tailwind CSS 4 + Custom Design System |
| **Database** | SQLite (WAL mode) |
| **Terminal** | xterm.js + WebSocket |
| **Dashboard** | GridStack.js |
| **Docker** | Bollard (API) + Docker CLI + Compose v2 |
| **Auth** | JWT + TOTP (2FA) + bcrypt |

---

## Development

### Prerequisites

- Rust 1.75+
- Node.js 20+
- Docker

### Run locally

```bash
# Frontend (hot reload)
cd frontend && npm install && npm run dev

# Backend
cargo run --bin dockpit-server
```

### Build Docker image

```bash
docker compose build
```

---

## Contributing

Contributions are welcome! Please open an issue or pull request on [GitHub](https://github.com/amslertec/dockpit).

## Sponsor

If you find DockPit useful, consider supporting the project:

- [GitHub Sponsors](https://github.com/sponsors/amslertec)
- [Buy Me a Coffee](https://www.buymeacoffee.com/amslertec)

## License

[MIT](LICENSE)
