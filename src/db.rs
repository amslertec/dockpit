use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::Mutex;

use crate::models::{EnvironmentInfo, UpdateCheckResult};

pub struct Database {
    conn: Mutex<Connection>,
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
        ")?;

        let db = Self {
            conn: Mutex::new(conn),
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
}
