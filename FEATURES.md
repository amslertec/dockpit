# DockPit — Feature Roadmap

A curated list of proposed features to enhance Docker server management, monitoring, and automation.
Check the boxes to mark features as planned or completed.

---

## Monitoring & Observability

### Live Resource Monitoring
- [ ] Real-time CPU, RAM, and network usage per container (sparkline charts)
- [ ] Stream Docker stats API via WebSocket for live updates
- [ ] Dashboard widget showing top 5 most resource-hungry containers
- [ ] Historical resource usage graphs (1h, 6h, 24h, 7d)
- [ ] Alerts when a container exceeds configurable CPU/RAM thresholds

### Container Event Log
- [ ] Central event timeline: start, stop, crash, OOM-kill, restart, image pull
- [ ] Filter events by server, container, event type, and time range
- [ ] "What happened last night?" view for debugging incidents
- [ ] Webhook/email notifications for critical events (crash, OOM)

### Health Check Dashboard
- [ ] Dedicated page for all containers with Docker health checks
- [ ] Health check result history with trend indicators
- [ ] Alert when a container restarts X times within Y minutes
- [ ] Health status overview widget for the dashboard

### Log Aggregation
- [ ] View logs from multiple containers simultaneously (merged view)
- [ ] Color-coded log lines per container in merged view
- [ ] Regex search within logs
- [ ] Log retention settings (auto-cleanup old logs)
- [ ] Log forwarding to external systems (syslog, Loki)

---

## Automation & Workflows

### Scheduled Actions
- [ ] Cron-like job scheduler with UI for creating and managing jobs
- [ ] Example: "Every Sunday at 03:00 recreate all containers with available updates"
- [ ] Example: "Every day at midnight run system prune"
- [ ] Job execution history with success/failure status
- [ ] Pause/resume individual scheduled jobs

### Auto-Recreate on Update
- [ ] Opt-in per container/stack: automatically pull + recreate when update detected
- [ ] Configurable maintenance window (only auto-update during off-hours)
- [ ] Automatic rollback if new container fails health check within X minutes
- [ ] Notification before and after auto-recreate

### Backup & Restore
- [ ] Backup Docker volumes (tar + compress)
- [ ] Scheduled backups to local directory, NFS, or S3-compatible storage
- [ ] One-click restore from backup
- [ ] Backup rotation (keep last N backups)
- [ ] Export/import stack configurations (compose + env + extra files)

---

## Multi-Server & Infrastructure

### Server Health Overview
- [ ] Host system metrics: CPU, RAM, disk usage, load average (not just Docker)
- [ ] Warning alerts at configurable thresholds (e.g., >90% disk)
- [ ] Host uptime and OS version display
- [ ] Network latency measurement between DockPit and remote agents
- [ ] Dashboard widget with server health summary

### Container Migration
- [ ] Move container/stack from Server A to Server B
- [ ] Volume data transfer via agent-to-agent communication
- [ ] Pre-migration validation (check target server has enough resources)
- [ ] Useful for server maintenance, scaling, or decommissioning

### Agent Auto-Discovery
- [ ] Scan local network for DockPit agents (mDNS/broadcast)
- [ ] "Scan Network" button instead of manual IP entry
- [ ] Auto-suggest discovered agents with one-click connect
- [ ] Agent version display and update notification

---

## Security

### Vulnerability Scanning
- [ ] Integrate Docker Scout or Trivy for image CVE scanning
- [ ] Severity overview per image (critical, high, medium, low)
- [ ] Dashboard widget: "X critical vulnerabilities across all images"
- [ ] Scheduled scans with notification on new CVEs
- [ ] Scan before deploying a new stack

### Audit Log
- [ ] Log every user action with timestamp, user, and details
- [ ] Actions: container start/stop, user created/deleted, settings changed, stack deployed
- [ ] Filterable audit log page with search
- [ ] Export as CSV/JSON
- [ ] Retention settings (auto-cleanup after X days)

### Secret Management
- [ ] View and manage Docker secrets
- [ ] Securely store environment variables in encrypted database
- [ ] Encrypted .env file management (not plaintext in compose files)
- [ ] Secret rotation reminders

---

## UX & Productivity

### Quick Command Palette (Ctrl+K)
- [ ] Global search bar triggered by keyboard shortcut
- [ ] Search across containers, stacks, servers, images, and actions
- [ ] Example: type "nginx" → shows all nginx containers across all servers
- [ ] Example: type "restart emby" → direct restart action
- [ ] Recent actions history in palette

### Notification Center
- [ ] Bell icon in topbar with unread notification count
- [ ] Notification types: container crash, update available, disk almost full, health check failed
- [ ] Browser push notifications (Notification API)
- [ ] Mark as read / dismiss / dismiss all
- [ ] Notification preferences per type (in-app, email, webhook)

### Favorites / Pinned Containers
- [ ] Pin frequently used containers for quick access
- [ ] Pinned containers widget on dashboard
- [ ] Quick actions on pinned items (start, stop, logs, terminal)
- [ ] Per-user pinned items

### Container Comparison
- [ ] Side-by-side comparison of two containers
- [ ] Compare: environment variables, ports, volumes, labels, image versions
- [ ] Diff view highlighting differences
- [ ] Useful for debugging "why does one work and the other doesn't?"

### Stack Templates
- [ ] Pre-built compose templates (Nginx Proxy Manager, Grafana, Portainer, Pi-hole, etc.)
- [ ] "New Stack from Template" button in stack creation
- [ ] Community template repository (import from URL)
- [ ] User-created templates (save current stack as template)
- [ ] Template variables (e.g., domain name, port) filled in via form

### Dashboard Customization
- [ ] More widget types (chart widgets, link widgets, note widgets)
- [ ] Widget themes/colors per widget
- [ ] Multiple dashboard tabs/pages
- [ ] Share dashboard layout with other users
- [ ] Import/export dashboard configuration

---

## Integration & API

### REST API
- [ ] Public REST API for all DockPit operations
- [ ] API key authentication (separate from user JWT)
- [ ] Swagger/OpenAPI documentation
- [ ] Rate limiting and CORS configuration

### External Integrations
- [ ] Grafana data source plugin
- [ ] Prometheus metrics endpoint (/metrics)
- [ ] Telegram bot notifications
- [ ] Home Assistant integration
- [ ] Uptime Kuma integration

---

## Priority Matrix

| Feature | Impact | Effort | Priority |
|---------|--------|--------|----------|
| Live Resource Monitoring | High | Medium | ⭐⭐⭐ |
| Quick Command Palette | High | Low | ⭐⭐⭐ |
| Scheduled Actions | High | Medium | ⭐⭐⭐ |
| Notification Center | High | Medium | ⭐⭐⭐ |
| Container Event Log | High | Low | ⭐⭐ |
| Health Check Dashboard | Medium | Low | ⭐⭐ |
| Audit Log | Medium | Low | ⭐⭐ |
| Vulnerability Scanning | High | Medium | ⭐⭐ |
| Stack Templates | Medium | Low | ⭐⭐ |
| Backup & Restore | High | High | ⭐⭐ |
| Server Health Overview | Medium | Medium | ⭐⭐ |
| Log Aggregation | Medium | Medium | ⭐ |
| Favorites / Pinned | Low | Low | ⭐ |
| Container Comparison | Low | Low | ⭐ |
| Auto-Recreate on Update | Medium | Medium | ⭐ |
| Agent Auto-Discovery | Low | Medium | ⭐ |
| Container Migration | Medium | High | ⭐ |
| Secret Management | Medium | Medium | ⭐ |
| REST API | Medium | Medium | ⭐ |
| External Integrations | Low | High | ⭐ |
