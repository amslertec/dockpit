use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::Mutex;

use crate::models::{EnvironmentInfo, UpdateCheckResult, ScheduledJob, NotificationInfo, ContainerEvent, VulnerabilityScan, AuditEntry, StackTemplate};

pub struct Database {
    conn: Mutex<Connection>,
    pub path: String,
}

impl Database {
    pub fn new(path: &str) -> Result<Self, rusqlite::Error> {
        // Ensure parent directory exists
        if let Some(parent) = Path::new(path).parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let conn = Connection::open(path)?;

        conn.execute_batch("
            PRAGMA journal_mode=WAL;
            PRAGMA foreign_keys=ON;
            PRAGMA busy_timeout=5000;
        ")?;

        let db = Self {
            conn: Mutex::new(conn),
            path: path.to_string(),
        };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        conn.execute_batch("
            CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                username TEXT UNIQUE NOT NULL,
                password_hash TEXT NOT NULL,
                role TEXT NOT NULL DEFAULT 'admin',
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS environments (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                url TEXT NOT NULL,
                agent_token TEXT,
                is_local INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS registries (
                registry TEXT PRIMARY KEY,
                username TEXT NOT NULL,
                password TEXT NOT NULL
            );

        ")?;

        // Migration: add totp_secret column
        conn.execute("ALTER TABLE users ADD COLUMN totp_secret TEXT", []).ok();
        conn.execute("ALTER TABLE users ADD COLUMN backup_codes TEXT", []).ok();

        // Migration: add hash column to audit_log
        conn.execute("ALTER TABLE audit_log ADD COLUMN hash TEXT", []).ok();

        // Migration: add color column to groups
        conn.execute("ALTER TABLE groups ADD COLUMN color TEXT DEFAULT '#6c5ce7'", []).ok();

        // Migration: add role column to users if not exists
        conn.execute("ALTER TABLE users ADD COLUMN role TEXT NOT NULL DEFAULT 'admin'", []).ok();

        // Migration: ensure first user is super_admin (if no super_admin exists yet)
        let has_super: i64 = conn.query_row(
            "SELECT COUNT(*) FROM users WHERE role = 'super_admin'", [], |row| row.get(0)
        ).unwrap_or(0);
        if has_super == 0 {
            conn.execute(
                "UPDATE users SET role = 'super_admin' WHERE rowid = (SELECT MIN(rowid) FROM users)",
                [],
            ).ok();
        }

        // Update check results
        conn.execute_batch("
            CREATE TABLE IF NOT EXISTS update_checks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                container_name TEXT NOT NULL,
                image TEXT NOT NULL,
                server_name TEXT NOT NULL,
                env_id TEXT NOT NULL,
                outdated INTEGER NOT NULL DEFAULT 0,
                current_id TEXT,
                latest_id TEXT,
                checked_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS notification_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                type TEXT NOT NULL,
                message TEXT NOT NULL,
                sent_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS stack_templates (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                category TEXT DEFAULT 'custom',
                compose_content TEXT NOT NULL,
                env_content TEXT,
                icon TEXT,
                is_default INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS audit_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT NOT NULL,
                action TEXT NOT NULL,
                target TEXT,
                details TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS vulnerability_scans (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                env_id TEXT NOT NULL,
                image TEXT NOT NULL,
                critical INTEGER NOT NULL DEFAULT 0,
                high INTEGER NOT NULL DEFAULT 0,
                medium INTEGER NOT NULL DEFAULT 0,
                low INTEGER NOT NULL DEFAULT 0,
                total INTEGER NOT NULL DEFAULT 0,
                cves_json TEXT,
                scanned_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS container_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                env_id TEXT NOT NULL,
                container_id TEXT,
                container_name TEXT,
                event_type TEXT NOT NULL,
                event_action TEXT NOT NULL,
                details TEXT,
                timestamp TEXT NOT NULL,
                UNIQUE(env_id, container_id, event_action, timestamp)
            );

            CREATE TABLE IF NOT EXISTS notifications (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                type TEXT NOT NULL,
                title TEXT NOT NULL,
                message TEXT NOT NULL,
                read INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS dashboard_configs (
                username TEXT PRIMARY KEY,
                config_json TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS scheduled_jobs (
                id TEXT PRIMARY KEY,
                env_id TEXT NOT NULL,
                job_type TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 0,
                interval_hours INTEGER NOT NULL DEFAULT 24,
                stack_name TEXT,
                last_run TEXT,
                next_run TEXT,
                last_result TEXT,
                last_message TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS container_snapshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                env_id TEXT NOT NULL,
                container_name TEXT NOT NULL,
                image TEXT NOT NULL,
                config_json TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
        ")?;

        conn.execute_batch("
            CREATE TABLE IF NOT EXISTS groups (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT UNIQUE NOT NULL,
                description TEXT,
                is_default INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS group_permissions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                group_id INTEGER NOT NULL,
                permission TEXT NOT NULL,
                UNIQUE(group_id, permission),
                FOREIGN KEY(group_id) REFERENCES groups(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS user_groups (
                user_id TEXT NOT NULL,
                group_id INTEGER NOT NULL,
                PRIMARY KEY(user_id, group_id),
                FOREIGN KEY(group_id) REFERENCES groups(id) ON DELETE CASCADE
            );
        ")?;

        // Indexes for frequently queried tables
        conn.execute_batch("
            CREATE INDEX IF NOT EXISTS idx_audit_log_created_at ON audit_log(created_at);
            CREATE INDEX IF NOT EXISTS idx_audit_log_username ON audit_log(username);
            CREATE INDEX IF NOT EXISTS idx_container_events_env_id ON container_events(env_id);
            CREATE INDEX IF NOT EXISTS idx_container_events_timestamp ON container_events(timestamp);
            CREATE INDEX IF NOT EXISTS idx_vulnerability_scans_env_id ON vulnerability_scans(env_id);
            CREATE INDEX IF NOT EXISTS idx_vulnerability_scans_image ON vulnerability_scans(image);
            CREATE INDEX IF NOT EXISTS idx_notifications_read ON notifications(read);
            CREATE INDEX IF NOT EXISTS idx_notifications_created_at ON notifications(created_at);
            CREATE INDEX IF NOT EXISTS idx_update_checks_env_id ON update_checks(env_id);
            CREATE INDEX IF NOT EXISTS idx_container_snapshots_name ON container_snapshots(container_name);
        ")?;

        conn.execute_batch("
            CREATE TABLE IF NOT EXISTS shell_snippets (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                container_name TEXT NOT NULL,
                title TEXT NOT NULL,
                command TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS alert_rules (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                event_match TEXT NOT NULL,
                action_type TEXT NOT NULL,
                config_json TEXT,
                last_triggered TEXT,
                trigger_count INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
        ")?;

        Ok(())
    }

    pub fn is_setup_complete(&self) -> bool {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))
            .unwrap_or(0);
        count > 0
    }

    pub fn create_user(&self, id: &str, username: &str, password_hash: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO users (id, username, password_hash, role) VALUES (?1, ?2, ?3, 'admin')",
            params![id, username, password_hash],
        )?;
        Ok(())
    }

    pub fn get_user_by_username(&self, username: &str) -> Option<(String, String, String)> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, username, password_hash FROM users WHERE username = ?1",
            params![username],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).ok()
    }

    // === Environment / Agent Management ===

    pub fn create_environment(&self, env: &EnvironmentInfo) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO environments (id, name, url, agent_token, is_local) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![env.id, env.name, env.url, env.agent_token, env.is_local as i32],
        )?;
        Ok(())
    }

    pub fn get_environments(&self) -> Vec<EnvironmentInfo> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT id, name, url, agent_token, is_local FROM environments ORDER BY is_local DESC, name")
            .unwrap();

        stmt.query_map([], |row| {
            let is_local: i32 = row.get(4)?;
            Ok(EnvironmentInfo {
                id: row.get(0)?,
                name: row.get(1)?,
                url: row.get(2)?,
                agent_token: row.get(3)?,
                is_local: is_local != 0,
                status: String::from("unknown"),
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
    }

    pub fn get_environment(&self, id: &str) -> Option<EnvironmentInfo> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, name, url, agent_token, is_local FROM environments WHERE id = ?1",
            params![id],
            |row| {
                let is_local: i32 = row.get(4)?;
                Ok(EnvironmentInfo {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    url: row.get(2)?,
                    agent_token: row.get(3)?,
                    is_local: is_local != 0,
                    status: String::from("unknown"),
                })
            },
        ).ok()
    }

    pub fn update_environment(&self, id: &str, name: &str, url: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE environments SET name = ?1, url = ?2 WHERE id = ?3",
            params![name, url, id],
        )?;
        Ok(())
    }

    pub fn delete_environment(&self, id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM environments WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn get_totp_secret(&self, username: &str) -> Option<String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT totp_secret FROM users WHERE username = ?1",
            params![username],
            |row| row.get::<_, Option<String>>(0),
        ).ok().flatten()
    }

    pub fn set_totp_secret(&self, username: &str, secret: Option<&str>) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE users SET totp_secret = ?1 WHERE username = ?2",
            params![secret, username],
        )?;
        Ok(())
    }

    pub fn get_backup_codes(&self, username: &str) -> Option<String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT backup_codes FROM users WHERE username = ?1",
            params![username],
            |row| row.get::<_, Option<String>>(0),
        ).ok().flatten()
    }

    pub fn set_backup_codes(&self, username: &str, codes: Option<&str>) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE users SET backup_codes = ?1 WHERE username = ?2",
            params![codes, username],
        )?;
        Ok(())
    }

    pub fn update_password(&self, username: &str, new_hash: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE users SET password_hash = ?1 WHERE username = ?2",
            params![new_hash, username],
        )?;
        Ok(())
    }

    // === Registry Management ===

    pub fn save_registry(&self, registry: &str, username: &str, password: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO registries (registry, username, password) VALUES (?1, ?2, ?3)",
            params![registry, username, password],
        )?;
        Ok(())
    }

    pub fn get_registries(&self) -> Vec<(String, String)> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT registry, username FROM registries ORDER BY registry").unwrap();
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    }

    pub fn get_registry_credentials(&self, registry: &str) -> Option<(String, String)> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT username, password FROM registries WHERE registry = ?1",
            params![registry],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).ok()
    }

    pub fn delete_registry(&self, registry: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM registries WHERE registry = ?1", params![registry])?;
        Ok(())
    }

    pub fn get_all_registry_credentials(&self) -> Vec<(String, String, String)> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT registry, username, password FROM registries").unwrap();
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    }

    pub fn ensure_local_environment(&self) {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM environments WHERE is_local = 1", [], |row| row.get(0))
            .unwrap_or(0);

        if count == 0 {
            let id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO environments (id, name, url, is_local) VALUES (?1, 'Local', 'local', 1)",
                params![id],
            ).ok();
        }
    }

    // === User Management ===

    pub fn list_users(&self) -> Vec<(String, String, String, String, bool)> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT id, username, role, created_at, totp_secret FROM users ORDER BY created_at")
            .unwrap();

        stmt.query_map([], |row| {
            let totp_secret: Option<String> = row.get(4)?;
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                totp_secret.is_some(),
            ))
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
    }

    pub fn create_user_with_role(&self, id: &str, username: &str, password_hash: &str, role: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO users (id, username, password_hash, role) VALUES (?1, ?2, ?3, ?4)",
            params![id, username, password_hash, role],
        )?;
        Ok(())
    }

    pub fn update_user_role(&self, id: &str, role: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE users SET role = ?1 WHERE id = ?2",
            params![role, id],
        )?;
        Ok(())
    }

    pub fn delete_user(&self, id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM users WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn get_user_role(&self, username: &str) -> Option<String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT role FROM users WHERE username = ?1",
            params![username],
            |row| row.get(0),
        ).ok()
    }

    // === Settings (key-value store) ===

    /// Create a consistent backup using SQLite's online backup API
    pub fn backup_to(&self, dest_path: &str) -> Result<(), String> {
        if let Some(parent) = Path::new(dest_path).parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("Cannot create backup dir: {}", e))?;
        }
        let conn = self.conn.lock().unwrap();
        let mut dest = Connection::open(dest_path).map_err(|e| e.to_string())?;
        let backup = rusqlite::backup::Backup::new(&conn, &mut dest).map_err(|e| e.to_string())?;
        backup.run_to_completion(100, std::time::Duration::from_millis(250), None)
            .map_err(|e| e.to_string())
    }

    /// Restore from a backup file by backing up from source into live connection
    pub fn restore_from(&self, source_path: &str) -> Result<(), String> {
        let src = Connection::open(source_path).map_err(|e| e.to_string())?;
        let mut conn = self.conn.lock().unwrap();
        let backup = rusqlite::backup::Backup::new(&src, &mut conn).map_err(|e| e.to_string())?;
        backup.run_to_completion(100, std::time::Duration::from_millis(250), None)
            .map_err(|e| e.to_string())
    }

    pub fn get_setting(&self, key: &str) -> Option<String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        ).ok()
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_all_settings(&self) -> Vec<(String, String)> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT key, value FROM settings ORDER BY key").unwrap();
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    }

    // === Dashboard Configs ===

    pub fn get_dashboard_config(&self, username: &str) -> Option<String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT config_json FROM dashboard_configs WHERE username = ?1",
            params![username],
            |row| row.get(0),
        ).ok()
    }

    pub fn save_dashboard_config(&self, username: &str, config_json: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO dashboard_configs (username, config_json, updated_at) VALUES (?1, ?2, datetime('now'))",
            params![username, config_json],
        )?;
        Ok(())
    }

    // === Container Snapshots ===

    pub fn save_container_snapshot(&self, env_id: &str, container_name: &str, image: &str, config_json: &str) {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO container_snapshots (env_id, container_name, image, config_json) VALUES (?1, ?2, ?3, ?4)",
            params![env_id, container_name, image, config_json],
        ).ok();
        // Keep only last 10 snapshots per container
        conn.execute(
            "DELETE FROM container_snapshots WHERE container_name = ?1 AND id NOT IN (SELECT id FROM container_snapshots WHERE container_name = ?1 ORDER BY id DESC LIMIT 10)",
            params![container_name],
        ).ok();
    }

    pub fn get_container_snapshots(&self, container_name: &str) -> Vec<(i64, String, String, String, String)> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, env_id, image, config_json, created_at FROM container_snapshots WHERE container_name = ?1 ORDER BY id DESC LIMIT 10"
        ).unwrap();
        stmt.query_map(params![container_name], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
        }).unwrap().filter_map(|r| r.ok()).collect()
    }

    pub fn get_snapshot_by_id(&self, id: i64) -> Option<(String, String, String)> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT container_name, image, config_json FROM container_snapshots WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).ok()
    }

    pub fn delete_snapshot(&self, id: i64) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM container_snapshots WHERE id = ?1", params![id])?;
        Ok(())
    }

    // === Shell Snippets ===

    pub fn get_snippets(&self, container_name: &str) -> Vec<(i64, String, String, String)> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, title, command, created_at FROM shell_snippets WHERE container_name = ?1 ORDER BY title").unwrap();
        stmt.query_map(params![container_name], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))
            .unwrap().filter_map(|r| r.ok()).collect()
    }

    pub fn save_snippet(&self, container_name: &str, title: &str, command: &str) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("INSERT INTO shell_snippets (container_name, title, command) VALUES (?1, ?2, ?3)", params![container_name, title, command])?;
        Ok(conn.last_insert_rowid())
    }

    pub fn delete_snippet(&self, id: i64) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM shell_snippets WHERE id = ?1", params![id])?;
        Ok(())
    }

    // === Groups & Permissions ===

    pub fn seed_default_group(&self) {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM groups WHERE is_default = 1", [], |row| row.get(0)).unwrap_or(0);
        if count == 0 {
            conn.execute("INSERT INTO groups (name, description, is_default) VALUES ('DockPit', 'Standardgruppe mit vollen Rechten', 1)", []).ok();
            // Add all permissions to default group
            if let Ok(group_id) = conn.query_row("SELECT id FROM groups WHERE is_default = 1", [], |row| row.get::<_, i64>(0)) {
                let all_perms = vec![
                    "page.dashboard", "page.containers", "page.stacks", "page.images", "page.volumes",
                    "page.networks", "page.monitoring", "page.health", "page.events", "page.updates",
                    "page.vulnerabilities", "page.audit", "page.host_terminal", "page.environments", "page.settings",
                    "action.container_start_stop", "action.container_restart", "action.container_recreate",
                    "action.container_delete", "action.container_logs", "action.container_terminal",
                    "action.stack_deploy_stop", "action.stack_create_delete", "action.stack_migrate",
                    "action.image_pull_delete", "action.volume_delete", "action.network_delete",
                    "action.backup", "action.user_management",
                ];
                for perm in all_perms {
                    conn.execute("INSERT OR IGNORE INTO group_permissions (group_id, permission) VALUES (?1, ?2)", params![group_id, perm]).ok();
                }
            }
        }
    }

    pub fn list_groups(&self) -> Vec<(i64, String, Option<String>, bool, String, String)> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, name, description, is_default, created_at, COALESCE(color, '#6c5ce7') FROM groups ORDER BY is_default DESC, name").unwrap();
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get::<_, i32>(3)? != 0, row.get(4)?, row.get(5)?)))
            .unwrap().filter_map(|r| r.ok()).collect()
    }

    pub fn create_group(&self, name: &str, description: Option<&str>, color: Option<&str>) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("INSERT INTO groups (name, description, color) VALUES (?1, ?2, ?3)", params![name, description, color.unwrap_or("#6c5ce7")])?;
        Ok(conn.last_insert_rowid())
    }

    pub fn update_group_info(&self, id: i64, name: &str, description: Option<&str>, color: Option<&str>) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("UPDATE groups SET name = ?1, description = ?2, color = ?3 WHERE id = ?4",
            params![name, description, color, id])?;
        Ok(())
    }

    pub fn delete_group(&self, id: i64) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        // Don't delete default group
        let is_default: i32 = conn.query_row("SELECT is_default FROM groups WHERE id = ?1", params![id], |row| row.get(0)).unwrap_or(1);
        if is_default != 0 { return Err(rusqlite::Error::QueryReturnedNoRows); }
        conn.execute("DELETE FROM group_permissions WHERE group_id = ?1", params![id])?;
        conn.execute("DELETE FROM user_groups WHERE group_id = ?1", params![id])?;
        conn.execute("DELETE FROM groups WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn get_group_permissions(&self, group_id: i64) -> Vec<String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT permission FROM group_permissions WHERE group_id = ?1 ORDER BY permission").unwrap();
        stmt.query_map(params![group_id], |row| row.get(0))
            .unwrap().filter_map(|r| r.ok()).collect()
    }

    pub fn set_group_permissions(&self, group_id: i64, permissions: &[String]) {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM group_permissions WHERE group_id = ?1", params![group_id]).ok();
        for perm in permissions {
            conn.execute("INSERT INTO group_permissions (group_id, permission) VALUES (?1, ?2)", params![group_id, perm]).ok();
        }
    }

    pub fn get_user_groups(&self, user_id: &str) -> Vec<(i64, String, String)> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT g.id, g.name, COALESCE(g.color, '#6c5ce7') FROM groups g JOIN user_groups ug ON g.id = ug.group_id WHERE ug.user_id = ?1 ORDER BY g.name").unwrap();
        stmt.query_map(params![user_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap().filter_map(|r| r.ok()).collect()
    }

    pub fn set_user_groups(&self, user_id: &str, group_ids: &[i64]) {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM user_groups WHERE user_id = ?1", params![user_id]).ok();
        for gid in group_ids {
            conn.execute("INSERT OR IGNORE INTO user_groups (user_id, group_id) VALUES (?1, ?2)", params![user_id, gid]).ok();
        }
    }

    pub fn get_user_permissions(&self, user_id: &str) -> Vec<String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT DISTINCT gp.permission FROM group_permissions gp JOIN user_groups ug ON gp.group_id = ug.group_id WHERE ug.user_id = ?1 ORDER BY gp.permission"
        ).unwrap();
        stmt.query_map(params![user_id], |row| row.get(0))
            .unwrap().filter_map(|r| r.ok()).collect()
    }

    pub fn get_group_members(&self, group_id: i64) -> Vec<(String, String)> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT u.id, u.username FROM users u JOIN user_groups ug ON u.id = ug.user_id WHERE ug.group_id = ?1 ORDER BY u.username").unwrap();
        stmt.query_map(params![group_id], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap().filter_map(|r| r.ok()).collect()
    }

    // === Alert Rules ===

    pub fn get_alert_rules(&self) -> Vec<(i64, String, bool, String, String, Option<String>, Option<String>, i64)> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, name, enabled, event_match, action_type, config_json, last_triggered, trigger_count FROM alert_rules ORDER BY name").unwrap();
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get::<_, i32>(2)? != 0, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?)))
            .unwrap().filter_map(|r| r.ok()).collect()
    }

    pub fn create_alert_rule(&self, name: &str, event_match: &str, action_type: &str, config_json: Option<&str>) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("INSERT INTO alert_rules (name, event_match, action_type, config_json) VALUES (?1, ?2, ?3, ?4)",
            params![name, event_match, action_type, config_json])?;
        Ok(())
    }

    pub fn update_alert_rule_enabled(&self, id: i64, enabled: bool) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("UPDATE alert_rules SET enabled = ?1 WHERE id = ?2", params![enabled as i32, id])?;
        Ok(())
    }

    pub fn delete_alert_rule(&self, id: i64) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM alert_rules WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn mark_alert_triggered(&self, id: i64) {
        let conn = self.conn.lock().unwrap();
        conn.execute("UPDATE alert_rules SET last_triggered = datetime('now'), trigger_count = trigger_count + 1 WHERE id = ?1", params![id]).ok();
    }

    pub fn get_recent_event_actions(&self, seconds: i64) -> Vec<(String, Option<String>, Option<String>, String)> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            &format!("SELECT env_id, container_id, container_name, event_action FROM container_events WHERE timestamp > datetime('now', '-{} seconds')", seconds)
        ).unwrap();
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))
            .unwrap().filter_map(|r| r.ok()).collect()
    }

    // === Update Checks ===

    pub fn save_update_check(
        &self,
        container_name: &str,
        image: &str,
        server_name: &str,
        env_id: &str,
        outdated: bool,
        current_id: Option<&str>,
        latest_id: Option<&str>,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO update_checks (container_name, image, server_name, env_id, outdated, current_id, latest_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![container_name, image, server_name, env_id, outdated as i32, current_id, latest_id],
        )?;
        Ok(())
    }

    pub fn get_latest_update_checks(&self) -> Vec<UpdateCheckResult> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT id, container_name, image, server_name, env_id, outdated, current_id, latest_id, checked_at FROM update_checks ORDER BY checked_at DESC")
            .unwrap();

        stmt.query_map([], |row| {
            let outdated: i32 = row.get(5)?;
            Ok(UpdateCheckResult {
                id: row.get(0)?,
                container_name: row.get(1)?,
                image: row.get(2)?,
                server_name: row.get(3)?,
                env_id: row.get(4)?,
                outdated: outdated != 0,
                current_id: row.get(6)?,
                latest_id: row.get(7)?,
                checked_at: row.get(8)?,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
    }

    pub fn clear_update_checks(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM update_checks", [])?;
        Ok(())
    }

    /// Mark a container as up-to-date after recreate (remove outdated entry)
    pub fn mark_container_updated(&self, container_name: &str) {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM update_checks WHERE container_name = ?1 OR container_name = ?2",
            params![container_name, container_name.trim_start_matches('/')],
        ).ok();
    }

    // === Scheduled Jobs ===

    pub fn get_scheduled_jobs(&self, env_id: Option<&str>) -> Vec<ScheduledJob> {
        let conn = self.conn.lock().unwrap();
        let (sql, param_values): (&str, Vec<String>) = if let Some(eid) = env_id {
            ("SELECT id, env_id, job_type, enabled, interval_hours, stack_name, last_run, next_run, last_result, last_message FROM scheduled_jobs WHERE env_id = ?1 ORDER BY created_at", vec![eid.to_string()])
        } else {
            ("SELECT id, env_id, job_type, enabled, interval_hours, stack_name, last_run, next_run, last_result, last_message FROM scheduled_jobs ORDER BY created_at", vec![])
        };
        let mut stmt = conn.prepare(sql).unwrap();
        let params: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|v| v as &dyn rusqlite::types::ToSql).collect();
        stmt.query_map(params.as_slice(), |row| {
            let enabled: i32 = row.get(3)?;
            Ok(ScheduledJob {
                id: row.get(0)?, env_id: row.get(1)?, job_type: row.get(2)?,
                enabled: enabled != 0, interval_hours: row.get(4)?,
                stack_name: row.get(5)?, last_run: row.get(6)?, next_run: row.get(7)?,
                last_result: row.get(8)?, last_message: row.get(9)?,
            })
        }).unwrap().filter_map(|r| r.ok()).collect()
    }

    pub fn get_due_jobs(&self) -> Vec<ScheduledJob> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, env_id, job_type, enabled, interval_hours, stack_name, last_run, next_run, last_result, last_message FROM scheduled_jobs WHERE enabled = 1 AND (next_run IS NULL OR next_run <= datetime('now'))"
        ).unwrap();
        stmt.query_map([], |row| {
            let enabled: i32 = row.get(3)?;
            Ok(ScheduledJob {
                id: row.get(0)?, env_id: row.get(1)?, job_type: row.get(2)?,
                enabled: enabled != 0, interval_hours: row.get(4)?,
                stack_name: row.get(5)?, last_run: row.get(6)?, next_run: row.get(7)?,
                last_result: row.get(8)?, last_message: row.get(9)?,
            })
        }).unwrap().filter_map(|r| r.ok()).collect()
    }

    pub fn create_scheduled_job(&self, job: &ScheduledJob) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO scheduled_jobs (id, env_id, job_type, enabled, interval_hours, stack_name, next_run) VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))",
            params![job.id, job.env_id, job.job_type, job.enabled as i32, job.interval_hours, job.stack_name],
        )?;
        Ok(())
    }

    pub fn update_scheduled_job(&self, id: &str, enabled: Option<bool>, interval_hours: Option<i32>) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        if let Some(e) = enabled {
            conn.execute("UPDATE scheduled_jobs SET enabled = ?1 WHERE id = ?2", params![e as i32, id])?;
        }
        if let Some(h) = interval_hours {
            conn.execute("UPDATE scheduled_jobs SET interval_hours = ?1 WHERE id = ?2", params![h, id])?;
        }
        Ok(())
    }

    pub fn delete_scheduled_job(&self, id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM scheduled_jobs WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn update_job_result(&self, id: &str, result: &str, message: &str, next_run: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE scheduled_jobs SET last_run = datetime('now'), last_result = ?1, last_message = ?2, next_run = ?3 WHERE id = ?4",
            params![result, message, next_run, id],
        )?;
        Ok(())
    }

    // === Stack Templates ===

    pub fn get_templates(&self) -> Vec<StackTemplate> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, name, description, category, compose_content, env_content, icon, is_default, created_at FROM stack_templates ORDER BY is_default DESC, name").unwrap();
        stmt.query_map([], |row| {
            let is_default: i32 = row.get(7)?;
            Ok(StackTemplate {
                id: row.get(0)?, name: row.get(1)?, description: row.get(2)?,
                category: row.get(3)?, compose_content: row.get(4)?,
                env_content: row.get(5)?, icon: row.get(6)?,
                is_default: is_default != 0, created_at: row.get(8)?,
            })
        }).unwrap().filter_map(|r| r.ok()).collect()
    }

    pub fn get_template(&self, id: &str) -> Option<StackTemplate> {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT id, name, description, category, compose_content, env_content, icon, is_default, created_at FROM stack_templates WHERE id = ?1", params![id], |row| {
            let is_default: i32 = row.get(7)?;
            Ok(StackTemplate {
                id: row.get(0)?, name: row.get(1)?, description: row.get(2)?,
                category: row.get(3)?, compose_content: row.get(4)?,
                env_content: row.get(5)?, icon: row.get(6)?,
                is_default: is_default != 0, created_at: row.get(8)?,
            })
        }).ok()
    }

    pub fn create_template(&self, t: &StackTemplate) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("INSERT INTO stack_templates (id, name, description, category, compose_content, env_content, icon, is_default) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![t.id, t.name, t.description, t.category, t.compose_content, t.env_content, t.icon, t.is_default as i32])?;
        Ok(())
    }

    pub fn update_template(&self, id: &str, name: &str, description: Option<&str>, compose_content: &str, env_content: Option<&str>) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("UPDATE stack_templates SET name = ?1, description = ?2, compose_content = ?3, env_content = ?4 WHERE id = ?5 AND is_default = 0",
            params![name, description, compose_content, env_content, id])?;
        Ok(())
    }

    pub fn delete_template(&self, id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM stack_templates WHERE id = ?1 AND is_default = 0", params![id])?;
        Ok(())
    }

    pub fn seed_default_templates(&self) {
        let defaults = vec![
            ("tpl-nginx-proxy", "Nginx Proxy Manager", "Reverse proxy with free SSL certificates", "proxy", "services:\n  npm:\n    image: jc21/nginx-proxy-manager:latest\n    container_name: nginx-proxy-manager\n    restart: unless-stopped\n    ports:\n      - \"80:80\"\n      - \"443:443\"\n      - \"81:81\"\n    volumes:\n      - npm_data:/data\n      - npm_letsencrypt:/etc/letsencrypt\n\nvolumes:\n  npm_data:\n  npm_letsencrypt:", "🔀"),
            ("tpl-portainer", "Portainer", "Docker management UI", "management", "services:\n  portainer:\n    image: portainer/portainer-ce:latest\n    container_name: portainer\n    restart: unless-stopped\n    ports:\n      - \"9443:9443\"\n    volumes:\n      - /var/run/docker.sock:/var/run/docker.sock:ro\n      - portainer_data:/data\n\nvolumes:\n  portainer_data:", "🐳"),
            ("tpl-uptime-kuma", "Uptime Kuma", "Self-hosted monitoring tool", "monitoring", "services:\n  uptime-kuma:\n    image: louislam/uptime-kuma:1\n    container_name: uptime-kuma\n    restart: unless-stopped\n    ports:\n      - \"3001:3001\"\n    volumes:\n      - uptime_data:/app/data\n\nvolumes:\n  uptime_data:", "📊"),
            ("tpl-postgres", "PostgreSQL", "Reliable SQL database", "database", "services:\n  postgres:\n    image: postgres:16-alpine\n    container_name: postgres\n    restart: unless-stopped\n    ports:\n      - \"5432:5432\"\n    environment:\n      - POSTGRES_USER=admin\n      - POSTGRES_PASSWORD=changeme\n      - POSTGRES_DB=mydb\n    volumes:\n      - pg_data:/var/lib/postgresql/data\n\nvolumes:\n  pg_data:", "🐘"),
            ("tpl-redis", "Redis", "In-memory cache and message broker", "database", "services:\n  redis:\n    image: redis:7-alpine\n    container_name: redis\n    restart: unless-stopped\n    ports:\n      - \"6379:6379\"\n    volumes:\n      - redis_data:/data\n    command: redis-server --appendonly yes\n\nvolumes:\n  redis_data:", "🔴"),
            ("tpl-grafana", "Grafana + Prometheus", "Monitoring stack with dashboards", "monitoring", "services:\n  grafana:\n    image: grafana/grafana:latest\n    container_name: grafana\n    restart: unless-stopped\n    ports:\n      - \"3000:3000\"\n    volumes:\n      - grafana_data:/var/lib/grafana\n    environment:\n      - GF_SECURITY_ADMIN_PASSWORD=admin\n\n  prometheus:\n    image: prom/prometheus:latest\n    container_name: prometheus\n    restart: unless-stopped\n    ports:\n      - \"9090:9090\"\n    volumes:\n      - prom_data:/prometheus\n\nvolumes:\n  grafana_data:\n  prom_data:", "📈"),
            ("tpl-wordpress", "WordPress", "CMS with MySQL database", "web", "services:\n  wordpress:\n    image: wordpress:latest\n    container_name: wordpress\n    restart: unless-stopped\n    ports:\n      - \"8080:80\"\n    environment:\n      - WORDPRESS_DB_HOST=wordpress-db\n      - WORDPRESS_DB_USER=wp\n      - WORDPRESS_DB_PASSWORD=changeme\n      - WORDPRESS_DB_NAME=wordpress\n    volumes:\n      - wp_data:/var/www/html\n    depends_on:\n      - wordpress-db\n\n  wordpress-db:\n    image: mysql:8.0\n    container_name: wordpress-db\n    restart: unless-stopped\n    environment:\n      - MYSQL_DATABASE=wordpress\n      - MYSQL_USER=wp\n      - MYSQL_PASSWORD=changeme\n      - MYSQL_ROOT_PASSWORD=rootpw\n    volumes:\n      - wp_db_data:/var/lib/mysql\n\nvolumes:\n  wp_data:\n  wp_db_data:", "📝"),
            ("tpl-nextcloud", "Nextcloud", "Self-hosted cloud storage", "storage", "services:\n  nextcloud:\n    image: nextcloud:latest\n    container_name: nextcloud\n    restart: unless-stopped\n    ports:\n      - \"8081:80\"\n    volumes:\n      - nc_data:/var/www/html\n    environment:\n      - SQLITE_DATABASE=nextcloud\n\nvolumes:\n  nc_data:", "☁️"),
            ("tpl-pihole", "Pi-hole", "Network-wide ad blocker", "network", "services:\n  pihole:\n    image: pihole/pihole:latest\n    container_name: pihole\n    restart: unless-stopped\n    ports:\n      - \"53:53/tcp\"\n      - \"53:53/udp\"\n      - \"8082:80\"\n    environment:\n      - WEBPASSWORD=changeme\n    volumes:\n      - pihole_etc:/etc/pihole\n      - pihole_dns:/etc/dnsmasq.d\n\nvolumes:\n  pihole_etc:\n  pihole_dns:", "🛡️"),
            ("tpl-watchtower", "Watchtower", "Automatic container updates", "management", "services:\n  watchtower:\n    image: containrrr/watchtower:latest\n    container_name: watchtower\n    restart: unless-stopped\n    volumes:\n      - /var/run/docker.sock:/var/run/docker.sock\n    environment:\n      - WATCHTOWER_CLEANUP=true\n      - WATCHTOWER_POLL_INTERVAL=86400", "🗼"),
        ];

        let conn = self.conn.lock().unwrap();
        for (id, name, desc, cat, compose, icon) in defaults {
            conn.execute("INSERT OR IGNORE INTO stack_templates (id, name, description, category, compose_content, icon, is_default) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1)",
                params![id, name, desc, cat, compose, icon]).ok();
        }
    }

    // === Audit Log ===

    pub fn log_audit(&self, username: &str, action: &str, target: Option<&str>, details: Option<&str>) {
        let conn = self.conn.lock().unwrap();
        // Get the hash of the last entry for chain integrity
        let prev_hash: String = conn.query_row(
            "SELECT hash FROM audit_log ORDER BY id DESC LIMIT 1",
            [],
            |row| row.get::<_, Option<String>>(0),
        ).ok().flatten().unwrap_or_else(|| "genesis".to_string());

        let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let payload = format!("{}|{}|{}|{}|{}|{}", prev_hash, timestamp, username, action, target.unwrap_or(""), details.unwrap_or(""));
        let hash = format!("{:x}", md5::compute(payload.as_bytes()));

        conn.execute(
            "INSERT INTO audit_log (username, action, target, details, created_at, hash) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![username, action, target, details, timestamp, hash],
        ).ok();
    }

    pub fn get_audit_log(&self, limit: i64, offset: i64, username: Option<&str>, action: Option<&str>) -> Vec<AuditEntry> {
        let conn = self.conn.lock().unwrap();
        let mut sql = "SELECT id, username, action, target, details, created_at, hash FROM audit_log WHERE 1=1".to_string();
        let mut param_values: Vec<String> = vec![];
        if let Some(u) = username {
            if !u.is_empty() { sql.push_str(&format!(" AND username = ?{}", param_values.len() + 1)); param_values.push(u.to_string()); }
        }
        if let Some(a) = action {
            if !a.is_empty() { sql.push_str(&format!(" AND action = ?{}", param_values.len() + 1)); param_values.push(a.to_string()); }
        }
        sql.push_str(&format!(" ORDER BY created_at DESC LIMIT ?{} OFFSET ?{}", param_values.len() + 1, param_values.len() + 2));
        param_values.push(limit.to_string());
        param_values.push(offset.to_string());

        let params: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|v| v as &dyn rusqlite::types::ToSql).collect();
        let mut stmt = conn.prepare(&sql).unwrap();
        stmt.query_map(params.as_slice(), |row| {
            Ok(AuditEntry {
                id: row.get(0)?, username: row.get(1)?, action: row.get(2)?,
                target: row.get(3)?, details: row.get(4)?, created_at: row.get(5)?,
                hash: row.get::<_, Option<String>>(6).ok().flatten(),
            })
        }).unwrap().filter_map(|r| r.ok()).collect()
    }

    pub fn get_audit_count(&self, username: Option<&str>, action: Option<&str>) -> i64 {
        let conn = self.conn.lock().unwrap();
        let mut sql = "SELECT COUNT(*) FROM audit_log WHERE 1=1".to_string();
        let mut param_values: Vec<String> = vec![];
        if let Some(u) = username {
            if !u.is_empty() { sql.push_str(&format!(" AND username = ?{}", param_values.len() + 1)); param_values.push(u.to_string()); }
        }
        if let Some(a) = action {
            if !a.is_empty() { sql.push_str(&format!(" AND action = ?{}", param_values.len() + 1)); param_values.push(a.to_string()); }
        }
        let params: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|v| v as &dyn rusqlite::types::ToSql).collect();
        conn.query_row(&sql, params.as_slice(), |row| row.get(0)).unwrap_or(0)
    }

    pub fn cleanup_old_audit(&self) {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM audit_log WHERE created_at < datetime('now', '-30 days')", []).ok();
    }

    // === Vulnerability Scans ===

    pub fn save_scan_result(&self, scan: &VulnerabilityScan) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO vulnerability_scans (env_id, image, critical, high, medium, low, total, cves_json, scanned_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'))",
            params![scan.env_id, scan.image, scan.critical, scan.high, scan.medium, scan.low, scan.total, scan.cves_json],
        )?;
        // Keep max 10 scans per image
        conn.execute(
            "DELETE FROM vulnerability_scans WHERE id NOT IN (SELECT id FROM vulnerability_scans WHERE env_id = ?1 AND image = ?2 ORDER BY scanned_at DESC LIMIT 10) AND env_id = ?1 AND image = ?2",
            params![scan.env_id, scan.image],
        )?;
        Ok(())
    }

    /// Get latest scan per image
    pub fn get_scan_results(&self, env_id: &str) -> Vec<VulnerabilityScan> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, env_id, image, critical, high, medium, low, total, cves_json, scanned_at FROM vulnerability_scans WHERE env_id = ?1 AND id IN (SELECT MAX(id) FROM vulnerability_scans WHERE env_id = ?1 GROUP BY image) ORDER BY critical DESC, high DESC"
        ).unwrap();
        stmt.query_map(params![env_id], |row| {
            Ok(VulnerabilityScan {
                id: row.get(0)?, env_id: row.get(1)?, image: row.get(2)?,
                critical: row.get(3)?, high: row.get(4)?, medium: row.get(5)?,
                low: row.get(6)?, total: row.get(7)?, cves_json: row.get(8)?,
                scanned_at: row.get(9)?,
            })
        }).unwrap().filter_map(|r| r.ok()).collect()
    }

    /// Get scan history for a specific image
    pub fn get_scan_history(&self, env_id: &str, image: &str) -> Vec<VulnerabilityScan> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, env_id, image, critical, high, medium, low, total, cves_json, scanned_at FROM vulnerability_scans WHERE env_id = ?1 AND image = ?2 ORDER BY scanned_at DESC LIMIT 10"
        ).unwrap();
        stmt.query_map(params![env_id, image], |row| {
            Ok(VulnerabilityScan {
                id: row.get(0)?, env_id: row.get(1)?, image: row.get(2)?,
                critical: row.get(3)?, high: row.get(4)?, medium: row.get(5)?,
                low: row.get(6)?, total: row.get(7)?, cves_json: row.get(8)?,
                scanned_at: row.get(9)?,
            })
        }).unwrap().filter_map(|r| r.ok()).collect()
    }

    // === Container Events ===

    pub fn save_events(&self, events: &[ContainerEvent]) {
        let conn = self.conn.lock().unwrap();
        for e in events {
            conn.execute(
                "INSERT OR IGNORE INTO container_events (env_id, container_id, container_name, event_type, event_action, details, timestamp) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![e.env_id, e.container_id, e.container_name, e.event_type, e.event_action, e.details, e.timestamp],
            ).ok();
        }
    }

    pub fn get_events(&self, env_id: &str, limit: i64, offset: i64) -> Vec<ContainerEvent> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, env_id, container_id, container_name, event_type, event_action, details, timestamp FROM container_events WHERE env_id = ?1 ORDER BY timestamp DESC LIMIT ?2 OFFSET ?3"
        ).unwrap();
        stmt.query_map(params![env_id, limit, offset], |row| {
            Ok(ContainerEvent {
                id: row.get(0)?, env_id: row.get(1)?, container_id: row.get(2)?,
                container_name: row.get(3)?, event_type: row.get(4)?,
                event_action: row.get(5)?, details: row.get(6)?, timestamp: row.get(7)?,
            })
        }).unwrap().filter_map(|r| r.ok()).collect()
    }

    pub fn get_events_count(&self, env_id: &str) -> i64 {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT COUNT(*) FROM container_events WHERE env_id = ?1", params![env_id], |row| row.get(0)).unwrap_or(0)
    }

    pub fn cleanup_old_events(&self) {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM container_events WHERE timestamp < datetime('now', '-7 days')", []).ok();
        // Remove noisy events — keep only meaningful actions
        conn.execute("DELETE FROM container_events WHERE event_action NOT IN ('start', 'stop', 'restart', 'oom')", []).ok();
    }

    // === Notifications ===

    pub fn create_notification(&self, ntype: &str, title: &str, message: &str) -> Result<(), rusqlite::Error> {
        // Check if notifications are enabled globally and for this type
        let enabled = self.get_setting("notif_enabled").unwrap_or_else(|| "true".to_string());
        if enabled == "false" { return Ok(()); }

        let type_key = format!("notif_{}", ntype);
        let type_enabled = self.get_setting(&type_key).unwrap_or_else(|| "true".to_string());
        if type_enabled == "false" { return Ok(()); }

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO notifications (type, title, message) VALUES (?1, ?2, ?3)",
            params![ntype, title, message],
        )?;
        // Keep only last 100 notifications
        conn.execute("DELETE FROM notifications WHERE id NOT IN (SELECT id FROM notifications ORDER BY created_at DESC LIMIT 100)", [])?;
        Ok(())
    }

    pub fn get_notifications(&self, limit: i64) -> Vec<NotificationInfo> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, type, title, message, read, created_at FROM notifications ORDER BY created_at DESC LIMIT ?1"
        ).unwrap();
        stmt.query_map(params![limit], |row| {
            let read: i32 = row.get(4)?;
            Ok(NotificationInfo {
                id: row.get(0)?, ntype: row.get(1)?, title: row.get(2)?,
                message: row.get(3)?, read: read != 0, created_at: row.get(5)?,
            })
        }).unwrap().filter_map(|r| r.ok()).collect()
    }

    pub fn get_unread_count(&self) -> i64 {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT COUNT(*) FROM notifications WHERE read = 0", [], |row| row.get(0)).unwrap_or(0)
    }

    pub fn mark_notification_read(&self, id: i64) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("UPDATE notifications SET read = 1 WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn mark_all_notifications_read(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("UPDATE notifications SET read = 1 WHERE read = 0", [])?;
        Ok(())
    }

    pub fn delete_notification(&self, id: i64) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM notifications WHERE id = ?1", params![id])?;
        Ok(())
    }
}
