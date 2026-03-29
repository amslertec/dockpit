# DockPit

A modern, futuristic Docker container management tool with a beautiful web UI. Manage containers, images, volumes, networks, and Docker Compose stacks across multiple servers — all from one dashboard.

![License](https://img.shields.io/badge/license-MIT-blue)
![Docker](https://img.shields.io/badge/docker-ready-blue)

## Features

### Multi-Server Management
- **Local & Remote** — Manage Docker on the local host and connect remote servers via the DockPit Agent
- **Environment Switching** — Quickly switch between servers from the topbar dropdown
- **Real-time Status** — Live status indicators for all connected servers

### Live Resource Monitoring
- **Real-time Stats** — CPU, RAM, Network I/O per container via WebSocket (updates every 2s)
- **Dedicated /monitoring Page** — Full table with sortable columns, color-coded progress bars, search
- **Dashboard Widget** — Compact Top 5 view for the Home page
- **Summary Cards** — Total CPU%, RAM usage, Network I/O, container count

### Quick Command Palette (Ctrl+K)
- **Global Search** — Search across pages, containers, stacks, and servers
- **Keyboard Navigation** — Arrow keys, Enter to select, Escape to close
- **Grouped Results** — Categories with icons and status badges
- **Instant Navigation** — Jump to any page or resource in seconds

### Vulnerability Scanner (Docker Scout)
- **Image CVE Scanning** — Scan all container images for known vulnerabilities using Docker Scout
- **SARIF Output Parsing** — Extracts CVE ID, severity, package, fixed version, description
- **Per-Image Results** — Expandable rows with CVE details, clickable links to NVD
- **Severity Summary** — Critical, High, Medium, Low counts with color coding
- **Scan History** — Up to 10 scans per image for trend tracking
- **Live Updates** — Results appear in real-time during active scans
- **Requires Docker Hub Login** — Free Personal plan sufficient (info banner on page)

### Health Check Dashboard
- **Dedicated /health Page** — Overview of all containers with Docker HEALTHCHECK
- **Status Overview** — Healthy (green), Unhealthy (red), Starting (yellow), No healthcheck (grey)
- **Health Details** — Check command, interval, retries, failing streak
- **Health Log** — Expandable rows showing last 5 check results with exit codes and output
- **Auto-Refresh** — Updates every 15 seconds
- **Per-Server View** — Filtered by currently selected environment

### Audit Log
- **Action Tracking** — Every user action logged with timestamp, user, action, target
- **17 Action Types** — Login, container ops, stack ops, user management, settings, scans
- **Filter & Search** — Filter by user and action type
- **30-Day Retention** — Auto-cleanup of old entries
- **Dedicated /audit Page** — Under "Management" in sidebar

### Container Event Log
- **Live Timeline** — Real-time container events: start, stop, restart, OOM
- **Dedicated /events Page** — Under "Management" in sidebar
- **Per-Server View** — Shows events only for the currently selected server
- **Auto-Collect** — Background collector runs every 30 seconds
- **7-Day Retention** — Events automatically cleaned up after 7 days
- **Filter & Paginate** — Filter by event type, paginated results

### Scheduled Jobs (Cron Automation)
- **Per-Server Jobs** — Schedule tasks per Docker server
- **3 Job Types** — Update Check, System Prune, Stack Redeploy
- **Configurable Intervals** — 1h, 6h, 12h, 24h, 48h, weekly
- **Run Now** — Manual trigger for any job
- **Result Tracking** — Success/error status with message history

### Notification Center
- **Bell Icon** — Topbar with unread badge count
- **Persistent Notifications** — Survive page reloads, stored in database
- **Auto-Generated** — Notifications for: job results, update checks, server connection problems
- **Preferences** — Per-type enable/disable in Profile settings

### Container Management
- **Full Lifecycle** — Start, stop, restart, recreate, and remove containers
- **Bulk Actions** — Select multiple containers for batch operations
- **Digest-based Update Detection** — Fast registry API checks without pulling images (works with private repos)
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
  - Resource Monitor — Live CPU/RAM/Network stats (Top 5 containers)

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
- **Timezone** — Configurable timezone for all date/time displays

### Prometheus Metrics
- **`/api/metrics` Endpoint** — Prometheus-compatible metrics in text exposition format
- **No Authentication** — Easy scraping without token configuration
- **Metrics Exposed:** Container counts, image/volume/network totals, health status, update status, stack status, users, notifications, scheduled jobs
- **Per-Environment** — Metrics labeled by environment/server name
- **Grafana Ready** — Add as Prometheus data source for long-term dashboards

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
