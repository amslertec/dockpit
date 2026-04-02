# DockPit

**Modern Docker container management with a beautiful web UI.**

Manage containers, images, volumes, networks, and Docker Compose stacks across multiple servers — all from one dashboard.

## Features

- **Multi-Server** — Manage local + remote Docker hosts via DockPit Agent
- **Live Monitoring** — Real-time CPU, RAM, Network I/O per container (WebSocket)
- **Container Rollback** — Automatic snapshots, one-click restore to any previous version
- **Container Migration** — Move containers/stacks between servers
- **Container Detail** — Full inspect view with env vars, ports, volumes, labels, networks
- **Host Terminal** — Direct shell access to Docker host servers
- **Shell Snippets** — Save and one-click execute commands per container
- **Smart Alerts** — Auto-fix rules (crash → restart, disk full → prune)
- **Group Permissions** — Granular per-page and per-action permission system
- **Container Events** — Live timeline of start, stop, restart, OOM events
- **Vulnerability Scanner** — CVE scanning with severity breakdown
- **Container Diff** — Compare snapshots to see what changed between versions
- **Scheduled Jobs** — Cron-like automation: update checks, system prune, stack redeploy
- **Stack Templates** — 10 pre-built + custom templates with icon picker
- **Network Auto-Discovery** — ARP-based agent scan across subnets
- **Server Pause/Resume** — Pause remote server connections
- **Notification Center** — In-app + Email (SMTP) + Webhooks (Slack, Discord, Teams)
- **Customizable Dashboard** — Drag-and-drop widgets, multiple tabs, import/export
- **Command Palette** — Ctrl+K to search containers, stacks, servers instantly
- **User Management** — 5 roles + group-based permissions with color-coded badges
- **Two-Factor Auth** — TOTP with QR code + 8 backup codes
- **Audit Log** — Hash-chain integrity, every action logged
- **Email Notifications** — Per-user SMTP with configurable preferences
- **Prometheus Metrics** — /api/metrics endpoint for Grafana
- **Dark & Light Mode** — Glassmorphism design
- **PWA** — Installable as mobile/desktop app
- **i18n** — English and German

## Quick Start

```yaml
services:
  dockpit:
    image: amslertec/dockpit:latest
    container_name: dockpit
    restart: unless-stopped
    network_mode: host
    pid: host
    privileged: true
    volumes:
      - dockpit_data:/data
      - /var/run/docker.sock:/var/run/docker.sock:ro
      - /var/docker/container:/stacks
    environment:
      - DOCKPIT_PORT=5533
      - DOCKPIT_JWT_SECRET=change-me-to-a-secure-random-string
      - DOCKPIT_STACKS_DIR=/stacks
      - DOCKPIT_HTTPS_PORT=5539

volumes:
  dockpit_data:
```

Open `http://your-server:5533` (HTTP) or `https://your-server:5539` (HTTPS) and create your admin account.

## Remote Server Agent

Deploy on any remote Docker host to manage it from DockPit:

```yaml
services:
  dockpit-agent:
    image: amslertec/dockpit-agent:latest
    container_name: dockpit-agent
    restart: unless-stopped
    pid: host
    privileged: true
    ports:
      - "5522:5522"
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro
      - /var/docker/container:/stacks
    environment:
      - AGENT_STACKS_DIR=/stacks
```

Then connect in DockPit under **Environments → Connect Remote Server** or use **Network Scan** to auto-discover agents.

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `DOCKPIT_PORT` | `5533` | HTTP port |
| `DOCKPIT_HTTPS_PORT` | `5539` | HTTPS port (auto-generated certificate) |
| `DOCKPIT_JWT_SECRET` | — | **Required.** JWT signing key (min. 16 chars) |
| `DOCKPIT_DB_PATH` | `/data/dockpit.db` | Database path |
| `DOCKPIT_STACKS_DIR` | `/stacks` | Compose stacks directory |

## Links

- [GitHub](https://github.com/amslertec/dockpit)
- [Releases](https://github.com/amslertec/dockpit/releases)
- [MIT License](https://github.com/amslertec/dockpit/blob/main/LICENSE)
