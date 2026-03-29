# DockPit Security & Performance Audit

## Executive Summary

DockPit has solid foundations (parameterized SQL, proper auth middleware, role-based access) but several critical gaps need addressing. This document outlines findings and a phased improvement plan.

---

## Critical Findings (Must Fix)

### 1. Hardcoded JWT Default Secret
- **File:** `src/auth.rs:12`
- **Risk:** Token forgery if env var not set
- **Fix:** Require `DOCKPIT_JWT_SECRET` or fail startup

### 2. No Rate Limiting on Login
- **File:** `src/handlers.rs` (login endpoint)
- **Risk:** Unlimited brute force on passwords
- **Fix:** Tower rate limiting middleware (5 attempts / 15 min)

### 3. Registry Passwords Stored in Plaintext
- **File:** `src/db.rs:58-62` (registries table)
- **Risk:** Credential theft from DB file
- **Fix:** Encrypt with AES-GCM, derive key from JWT secret

### 4. XSS via Container Logs (@html)
- **File:** `frontend/src/routes/containers/[id]/logs/+page.svelte:208`
- **Risk:** JavaScript injection via Docker container logs
- **Fix:** Sanitize ANSI parser output, remove `@html` or use DOMPurify

### 5. 24-Hour JWT Token Lifetime
- **File:** `src/auth.rs:20`
- **Risk:** Stolen tokens valid for 24 hours
- **Fix:** Reduce to 1-2 hours, add refresh token mechanism

---

## High Findings

### 6. Single Database Connection (Mutex)
- **File:** `src/db.rs:7-9`
- **Impact:** All requests serialize on DB access → slow under load
- **Fix:** Add `PRAGMA busy_timeout`, or migrate to connection pool

### 7. No CSRF Protection
- **Impact:** Cross-site request forgery possible
- **Fix:** Add CSRF tokens to state-changing endpoints

### 8. Missing Database Indexes
- **Impact:** Slow queries on audit_log, container_events
- **Fix:** Add indexes on frequently queried columns

### 9. No Docker Operation Timeouts
- **File:** `src/docker.rs` (start/stop/remove)
- **Impact:** Hung Docker socket blocks request indefinitely
- **Fix:** Wrap all Docker calls in `tokio::time::timeout`

### 10. WebSocket Token in URL
- **Impact:** Token visible in logs, browser history
- **Fix:** Use short-lived one-time tokens

---

## Medium Findings

### 11. No TOTP Backup Codes
### 12. No Cache-Control Headers on Static Assets
### 13. No Environment List Caching
### 14. Sequential Docker Operations (some already parallelized)
### 15. YAML Content Not Validated (stack editor)
### 16. Audit Log Tampering Possible

---

## Low Findings

### 17. Polling Intervals Could Be Optimized
### 18. No Lazy Loading for Frontend Routes
### 19. Error Messages Could Leak Information

---

## Implementation Phases

### Phase 1: Critical Security (Immediate)
**Estimated effort: 2-4 hours**

1. Remove hardcoded JWT secret — fail startup if not set
2. Add rate limiting on `/api/login` and `/api/setup` (Tower middleware)
3. Encrypt registry passwords in DB (AES-GCM)
4. Fix XSS in logs viewer — sanitize @html output
5. Reduce JWT expiry to 2 hours

### Phase 2: High Security (This Week)
**Estimated effort: 4-6 hours**

6. Add `PRAGMA busy_timeout = 5000` for SQLite
7. Add CSRF token validation
8. Add database indexes for audit_log, container_events, vulnerability_scans
9. Add timeouts to all Docker operations (30s default)
10. Implement one-time WebSocket tokens

### Phase 3: Performance (Next Week)
**Estimated effort: 3-4 hours**

11. Add Cache-Control headers for static assets (immutable for JS/CSS)
12. Cache environment list in memory (5 min TTL)
13. Cache registry credentials during update checks
14. Parallelize remaining sequential Docker operations
15. Optimize polling intervals (logs: 5s → 10s, events: 30s → 60s)

### Phase 4: Hardening (Ongoing)
**Estimated effort: 4-6 hours**

16. Add TOTP backup codes
17. Implement audit log integrity (hash chain)
18. Add YAML content validation (whitelist keys, reject anchors)
19. Implement proper refresh token rotation
20. Add Content-Security-Policy headers

---

## Metrics After Implementation

| Metric | Current | Target |
|--------|---------|--------|
| Critical vulnerabilities | 5 | 0 |
| High vulnerabilities | 5 | 0 |
| Login brute force protection | None | 5 attempts / 15 min |
| JWT token lifetime | 24 hours | 2 hours |
| DB query performance (audit) | Full scan | Indexed |
| Static asset caching | None | 1 year (immutable) |
| Docker operation timeout | None | 30 seconds |
