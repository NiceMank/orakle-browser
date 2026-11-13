use crate::error::Result;
use rusqlite::{params, Connection, Transaction};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Space {
    pub id: String,
    pub name: String,
    pub color: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProfileRecord {
    pub id: String,
    pub name: String,
    pub data_dir: String,
    pub is_ephemeral: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TabGroup {
    pub id: String,
    pub space_id: String,
    pub name: String,
    pub color: String,
    pub collapsed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TabRecord {
    pub id: String,
    pub space_id: String,
    pub group_id: Option<String>,
    pub url: String,
    pub title: String,
    pub is_pinned: bool,
    pub is_suspended: bool,
    pub is_incognito: bool,
    pub position: i32,
    pub scroll_y: i32,
    pub last_active_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionRecord {
    pub id: String,
    pub name: String,
    pub space_id: String,
    pub snapshot_json: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DownloadRecord {
    pub id: String,
    pub url: String,
    pub filename: String,
    pub save_path: String,
    pub status: String,
    pub bytes_received: i64,
    pub total_bytes: i64,
    pub started_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DomainPermission {
    pub origin: String,
    pub permission: String,
    pub decision: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NoteEntry {
    pub id: String,
    pub title: String,
    pub content: String,
    pub url: Option<String>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HistoryEntry {
    pub id: Option<i64>,
    pub url: String,
    pub title: String,
    pub visited_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BookmarkEntry {
    pub id: String,
    pub title: String,
    pub url: String,
    pub category: String,
    pub created_at: i64,
}

pub struct StorageEngine {
    conn: Connection,
}

impl StorageEngine {
    /// Ouvre ou crée une base SQLite locale avec les paramètres optimisés WAL et intégrité référentielle
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        let engine = Self { conn };
        engine.configure_pragmas()?;
        engine.apply_migrations()?;
        Ok(engine)
    }

    /// Ouvre une base en mémoire (idéal pour les tests unitaires et sessions privées pures)
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let engine = Self { conn };
        engine.configure_pragmas()?;
        engine.apply_migrations()?;
        Ok(engine)
    }

    /// Applique les réglages de performance, atomicité et concurrence SQLite
    fn configure_pragmas(&self) -> Result<()> {
        self.conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;
             PRAGMA temp_store = MEMORY;",
        )?;
        Ok(())
    }

    /// Vérifie et applique les migrations de schéma versionnées
    fn apply_migrations(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at INTEGER NOT NULL
            );",
        )?;

        let current_version: i64 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if current_version < 1 {
            let tx = self.conn.unchecked_transaction()?;
            tx.execute_batch(
                "CREATE TABLE spaces (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    color TEXT NOT NULL,
                    created_at INTEGER NOT NULL
                );

                CREATE TABLE tabs (
                    id TEXT PRIMARY KEY,
                    space_id TEXT NOT NULL REFERENCES spaces(id) ON DELETE CASCADE,
                    url TEXT NOT NULL,
                    title TEXT NOT NULL,
                    is_pinned INTEGER NOT NULL DEFAULT 0,
                    is_suspended INTEGER NOT NULL DEFAULT 0,
                    position INTEGER NOT NULL,
                    scroll_y INTEGER NOT NULL DEFAULT 0,
                    last_active_at INTEGER NOT NULL
                );
                CREATE INDEX idx_tabs_space ON tabs(space_id, position);

                CREATE TABLE history (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    url TEXT NOT NULL,
                    title TEXT NOT NULL,
                    visited_at INTEGER NOT NULL
                );
                CREATE INDEX idx_history_visited_at ON history(visited_at DESC);
                CREATE INDEX idx_history_url ON history(url);

                CREATE TABLE bookmarks (
                    id TEXT PRIMARY KEY,
                    title TEXT NOT NULL,
                    url TEXT NOT NULL,
                    category TEXT NOT NULL,
                    created_at INTEGER NOT NULL
                );
                CREATE INDEX idx_bookmarks_category ON bookmarks(category);

                CREATE TABLE settings (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                );

                INSERT INTO schema_migrations (version, applied_at) VALUES (1, strftime('%s', 'now'));",
            )?;
            tx.commit()?;
        }

        if current_version < 2 {
            let tx = self.conn.unchecked_transaction()?;
            tx.execute_batch(
                "CREATE TABLE IF NOT EXISTS profiles (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    data_dir TEXT NOT NULL,
                    is_ephemeral INTEGER NOT NULL DEFAULT 0,
                    created_at INTEGER NOT NULL
                );

                CREATE TABLE IF NOT EXISTS tab_groups (
                    id TEXT PRIMARY KEY,
                    space_id TEXT NOT NULL REFERENCES spaces(id) ON DELETE CASCADE,
                    name TEXT NOT NULL,
                    color TEXT NOT NULL,
                    collapsed INTEGER NOT NULL DEFAULT 0
                );

                CREATE TABLE IF NOT EXISTS sessions (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    space_id TEXT NOT NULL,
                    snapshot_json TEXT NOT NULL,
                    created_at INTEGER NOT NULL
                );

                CREATE TABLE IF NOT EXISTS downloads (
                    id TEXT PRIMARY KEY,
                    url TEXT NOT NULL,
                    filename TEXT NOT NULL,
                    save_path TEXT NOT NULL,
                    status TEXT NOT NULL,
                    bytes_received INTEGER NOT NULL DEFAULT 0,
                    started_at INTEGER NOT NULL
                );

                CREATE TABLE IF NOT EXISTS notes (
                    id TEXT PRIMARY KEY,
                    title TEXT NOT NULL,
                    content TEXT NOT NULL,
                    url TEXT,
                    updated_at INTEGER NOT NULL
                );

                ALTER TABLE tabs ADD COLUMN group_id TEXT;
                ALTER TABLE tabs ADD COLUMN is_incognito INTEGER NOT NULL DEFAULT 0;

                INSERT INTO schema_migrations (version, applied_at) VALUES (2, strftime('%s', 'now'));",
            )?;
            tx.commit()?;
        }

        if current_version < 3 {
            let tx = self.conn.unchecked_transaction()?;
            tx.execute_batch(
                "ALTER TABLE downloads ADD COLUMN total_bytes INTEGER NOT NULL DEFAULT 0;

                CREATE TABLE IF NOT EXISTS domain_permissions (
                    origin TEXT NOT NULL,
                    permission TEXT NOT NULL,
                    decision TEXT NOT NULL,
                    updated_at INTEGER NOT NULL,
                    PRIMARY KEY (origin, permission)
                );

                INSERT INTO schema_migrations (version, applied_at) VALUES (3, strftime('%s', 'now'));",
            )?;
            tx.commit()?;
        }

        Ok(())
    }

    /// Exécute un contrôle d'intégrité formel SQLite
    pub fn check_integrity(&self) -> Result<bool> {
        let result: String = self
            .conn
            .query_row("PRAGMA integrity_check;", [], |row| row.get(0))?;
        Ok(result == "ok")
    }

    /// Force un checkpoint WAL pour synchroniser les pages disques
    pub fn checkpoint_wal(&self) -> Result<()> {
        self.conn
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(())
    }

    // --- Opérations Espaces (Spaces) ---
    pub fn insert_space(&self, space: &Space) -> Result<()> {
        self.conn.execute(
            "INSERT INTO spaces (id, name, color, created_at) VALUES (?1, ?2, ?3, ?4);",
            params![space.id, space.name, space.color, space.created_at],
        )?;
        Ok(())
    }

    pub fn get_spaces(&self) -> Result<Vec<Space>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, color, created_at FROM spaces ORDER BY created_at ASC;")?;
        let rows = stmt.query_map([], |row| {
            Ok(Space {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?;

        let mut spaces = Vec::new();
        for r in rows {
            spaces.push(r?);
        }
        Ok(spaces)
    }

    // --- Opérations Onglets (Tabs, Groupes & Tab Discarding) ---
    pub fn upsert_tab(&self, tab: &TabRecord) -> Result<()> {
        self.conn.execute(
            "INSERT INTO tabs (id, space_id, group_id, url, title, is_pinned, is_suspended, is_incognito, position, scroll_y, last_active_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(id) DO UPDATE SET
                space_id = excluded.space_id,
                group_id = excluded.group_id,
                url = excluded.url,
                title = excluded.title,
                is_pinned = excluded.is_pinned,
                is_suspended = excluded.is_suspended,
                is_incognito = excluded.is_incognito,
                position = excluded.position,
                scroll_y = excluded.scroll_y,
                last_active_at = excluded.last_active_at;",
            params![
                tab.id,
                tab.space_id,
                tab.group_id,
                tab.url,
                tab.title,
                if tab.is_pinned { 1 } else { 0 },
                if tab.is_suspended { 1 } else { 0 },
                if tab.is_incognito { 1 } else { 0 },
                tab.position,
                tab.scroll_y,
                tab.last_active_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_tabs_for_space(&self, space_id: &str) -> Result<Vec<TabRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, space_id, group_id, url, title, is_pinned, is_suspended, is_incognito, position, scroll_y, last_active_at
             FROM tabs WHERE space_id = ?1 ORDER BY position ASC;",
        )?;

        let rows = stmt.query_map(params![space_id], |row| {
            Ok(TabRecord {
                id: row.get(0)?,
                space_id: row.get(1)?,
                group_id: row.get(2)?,
                url: row.get(3)?,
                title: row.get(4)?,
                is_pinned: row.get::<_, i32>(5)? != 0,
                is_suspended: row.get::<_, i32>(6)? != 0,
                is_incognito: row.get::<_, i32>(7)? != 0,
                position: row.get(8)?,
                scroll_y: row.get(9)?,
                last_active_at: row.get(10)?,
            })
        })?;

        let mut tabs = Vec::new();
        for r in rows {
            tabs.push(r?);
        }
        Ok(tabs)
    }

    pub fn delete_tab(&self, tab_id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM tabs WHERE id = ?1;", params![tab_id])?;
        Ok(())
    }

    pub fn upsert_tab_group(&self, g: &TabGroup) -> Result<()> {
        self.conn.execute(
            "INSERT INTO tab_groups (id, space_id, name, color, collapsed) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET name=excluded.name, color=excluded.color, collapsed=excluded.collapsed;",
            params![g.id, g.space_id, g.name, g.color, if g.collapsed { 1 } else { 0 }],
        )?;
        Ok(())
    }

    pub fn get_tab_groups(&self, space_id: &str) -> Result<Vec<TabGroup>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, space_id, name, color, collapsed FROM tab_groups WHERE space_id = ?1;",
        )?;
        let rows = stmt.query_map(params![space_id], |row| {
            Ok(TabGroup {
                id: row.get(0)?,
                space_id: row.get(1)?,
                name: row.get(2)?,
                color: row.get(3)?,
                collapsed: row.get::<_, i32>(4)? != 0,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    // --- Opérations Historique & Batch ---
    pub fn insert_history_entry(&self, entry: &HistoryEntry) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO history (url, title, visited_at) VALUES (?1, ?2, ?3);",
            params![entry.url, entry.title, entry.visited_at],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Insertion d'un lot d'entrées d'historique au sein d'une transaction unique (optimisation batch)
    pub fn insert_history_batch(&mut self, entries: &[HistoryEntry]) -> Result<usize> {
        let tx: Transaction = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO history (url, title, visited_at) VALUES (?1, ?2, ?3);",
            )?;
            for e in entries {
                stmt.execute(params![e.url, e.title, e.visited_at])?;
            }
        }
        tx.commit()?;
        Ok(entries.len())
    }

    pub fn get_history(&self, limit: usize) -> Result<Vec<HistoryEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, url, title, visited_at FROM history ORDER BY visited_at DESC LIMIT ?1;",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(HistoryEntry {
                id: Some(row.get(0)?),
                url: row.get(1)?,
                title: row.get(2)?,
                visited_at: row.get(3)?,
            })
        })?;

        let mut history = Vec::new();
        for r in rows {
            history.push(r?);
        }
        Ok(history)
    }

    pub fn clear_history(&self) -> Result<()> {
        self.conn.execute("DELETE FROM history;", [])?;
        Ok(())
    }

    // --- Opérations Favoris (Bookmarks) ---
    pub fn upsert_bookmark(&self, b: &BookmarkEntry) -> Result<()> {
        self.conn.execute(
            "INSERT INTO bookmarks (id, title, url, category, created_at) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET title=excluded.title, url=excluded.url, category=excluded.category;",
            params![b.id, b.title, b.url, b.category, b.created_at],
        )?;
        Ok(())
    }

    pub fn get_bookmarks(&self) -> Result<Vec<BookmarkEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, url, category, created_at FROM bookmarks ORDER BY created_at DESC;",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(BookmarkEntry {
                id: row.get(0)?,
                title: row.get(1)?,
                url: row.get(2)?,
                category: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn delete_bookmark(&self, id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM bookmarks WHERE id = ?1;", params![id])?;
        Ok(())
    }

    // --- Opérations Sessions ---
    pub fn save_session(&self, s: &SessionRecord) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sessions (id, name, space_id, snapshot_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET name=excluded.name, snapshot_json=excluded.snapshot_json;",
            params![s.id, s.name, s.space_id, s.snapshot_json, s.created_at],
        )?;
        Ok(())
    }

    pub fn get_sessions(&self) -> Result<Vec<SessionRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, space_id, snapshot_json, created_at FROM sessions ORDER BY created_at DESC;",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(SessionRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                space_id: row.get(2)?,
                snapshot_json: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn delete_session(&self, id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM sessions WHERE id = ?1;", params![id])?;
        Ok(())
    }

    // --- Opérations Téléchargements (Downloads) ---
    pub fn upsert_download(&self, d: &DownloadRecord) -> Result<()> {
        self.conn.execute(
            "INSERT INTO downloads (id, url, filename, save_path, status, bytes_received, total_bytes, started_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET status=excluded.status, bytes_received=excluded.bytes_received, total_bytes=excluded.total_bytes, save_path=excluded.save_path;",
            params![d.id, d.url, d.filename, d.save_path, d.status, d.bytes_received, d.total_bytes, d.started_at],
        )?;
        Ok(())
    }

    pub fn get_downloads(&self) -> Result<Vec<DownloadRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, url, filename, save_path, status, bytes_received, total_bytes, started_at FROM downloads ORDER BY started_at DESC LIMIT 100;",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(DownloadRecord {
                id: row.get(0)?,
                url: row.get(1)?,
                filename: row.get(2)?,
                save_path: row.get(3)?,
                status: row.get(4)?,
                bytes_received: row.get(5)?,
                total_bytes: row.get(6)?,
                started_at: row.get(7)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn delete_download(&self, id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM downloads WHERE id = ?1;", params![id])?;
        Ok(())
    }

    // --- Opérations Notes ---
    pub fn upsert_note(&self, n: &NoteEntry) -> Result<()> {
        self.conn.execute(
            "INSERT INTO notes (id, title, content, url, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET title=excluded.title, content=excluded.content, url=excluded.url, updated_at=excluded.updated_at;",
            params![n.id, n.title, n.content, n.url, n.updated_at],
        )?;
        Ok(())
    }

    pub fn get_notes(&self) -> Result<Vec<NoteEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, content, url, updated_at FROM notes ORDER BY updated_at DESC;",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(NoteEntry {
                id: row.get(0)?,
                title: row.get(1)?,
                content: row.get(2)?,
                url: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn delete_note(&self, id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM notes WHERE id = ?1;", params![id])?;
        Ok(())
    }

    // --- Opérations Profils & Paramètres ---
    pub fn upsert_profile(&self, p: &ProfileRecord) -> Result<()> {
        self.conn.execute(
            "INSERT INTO profiles (id, name, data_dir, is_ephemeral, created_at) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET name=excluded.name, data_dir=excluded.data_dir;",
            params![p.id, p.name, p.data_dir, if p.is_ephemeral { 1 } else { 0 }, p.created_at],
        )?;
        Ok(())
    }

    pub fn get_profiles(&self) -> Result<Vec<ProfileRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, data_dir, is_ephemeral, created_at FROM profiles ORDER BY created_at ASC;",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ProfileRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                data_dir: row.get(2)?,
                is_ephemeral: row.get::<_, i32>(3)? != 0,
                created_at: row.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value;",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM settings WHERE key = ?1;")?;
        let mut rows = stmt.query(params![key])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    pub fn get_all_settings(&self) -> Result<Vec<(String, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT key, value FROM settings ORDER BY key ASC;")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    // --- Opérations Permissions par Domaine ---
    pub fn set_domain_permission(&self, p: &DomainPermission) -> Result<()> {
        self.conn.execute(
            "INSERT INTO domain_permissions (origin, permission, decision, updated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(origin, permission) DO UPDATE SET decision=excluded.decision, updated_at=excluded.updated_at;",
            params![p.origin, p.permission, p.decision, p.updated_at],
        )?;
        Ok(())
    }

    pub fn get_domain_permissions(&self) -> Result<Vec<DomainPermission>> {
        let mut stmt = self.conn.prepare(
            "SELECT origin, permission, decision, updated_at FROM domain_permissions ORDER BY updated_at DESC;",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(DomainPermission {
                origin: row.get(0)?,
                permission: row.get(1)?,
                decision: row.get(2)?,
                updated_at: row.get(3)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn delete_domain_permission(&self, origin: &str, permission: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM domain_permissions WHERE origin = ?1 AND permission = ?2;",
            params![origin, permission],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_sqlite_integrity_and_pragmas() {
        let engine = StorageEngine::open_in_memory().expect("open memory db");
        assert!(engine.check_integrity().expect("integrity check"));
    }

    #[test]
    fn test_foreign_key_constraint_rollback() {
        let engine = StorageEngine::open_in_memory().expect("open memory db");

        let bad_tab = TabRecord {
            id: "tab-orphan".to_string(),
            space_id: "space-does-not-exist".to_string(),
            group_id: None,
            url: "https://bad.com".to_string(),
            title: "Bad".to_string(),
            is_pinned: false,
            is_suspended: false,
            is_incognito: false,
            position: 0,
            scroll_y: 0,
            last_active_at: 1000,
        };

        let result = engine.upsert_tab(&bad_tab);
        assert!(
            result.is_err(),
            "L'insertion d'un onglet orphelin doit être rejetée par PRAGMA foreign_keys"
        );
    }

    #[test]
    fn test_tab_discarding_cycle() {
        let engine = StorageEngine::open_in_memory().expect("open memory db");

        let space = Space {
            id: "space-1".to_string(),
            name: "Personnel".to_string(),
            color: "#2f81f7".to_string(),
            created_at: 1000,
        };
        engine.insert_space(&space).unwrap();

        let mut tab = TabRecord {
            id: "tab-1".to_string(),
            space_id: "space-1".to_string(),
            group_id: None,
            url: "https://doc.rust-lang.org".to_string(),
            title: "Rust Book".to_string(),
            is_pinned: false,
            is_suspended: false,
            is_incognito: false,
            position: 0,
            scroll_y: 1250,
            last_active_at: 2000,
        };
        engine.upsert_tab(&tab).unwrap();

        // Simulation suspension (Tab Discarding)
        tab.is_suspended = true;
        engine.upsert_tab(&tab).unwrap();

        let tabs = engine.get_tabs_for_space("space-1").unwrap();
        assert_eq!(tabs.len(), 1);
        assert!(tabs[0].is_suspended);
        assert_eq!(tabs[0].scroll_y, 1250);

        // Simulation réactivation
        tab.is_suspended = false;
        engine.upsert_tab(&tab).unwrap();
        let tabs_reactivated = engine.get_tabs_for_space("space-1").unwrap();
        assert!(!tabs_reactivated[0].is_suspended);
    }

    #[test]
    fn test_benchmark_batch_vs_single_transactions() {
        let mut engine = StorageEngine::open_in_memory().expect("open memory db");

        // 1. Benchmark Batch Transaction (1 000 entrées)
        let entries: Vec<HistoryEntry> = (0..1000)
            .map(|i| HistoryEntry {
                id: None,
                url: format!("https://example.com/page-{}", i),
                title: format!("Page {}", i),
                visited_at: 1731300000 + i,
            })
            .collect();

        let t_start_batch = Instant::now();
        engine.insert_history_batch(&entries).unwrap();
        let dur_batch = t_start_batch.elapsed();
        let per_item_batch = dur_batch.as_micros() as f64 / 1000.0;

        println!(
            "  [BENCHMARK SQLITE BATCH] 1 000 entrées insérées en {:?} ({:.2} µs/entrée)",
            dur_batch, per_item_batch
        );
        assert!(
            per_item_batch < 50.0,
            "Le batch WAL doit être ultra-rapide (< 50 µs par entrée)"
        );

        let history = engine.get_history(2000).unwrap();
        assert_eq!(history.len(), 1000);
    }

    #[test]
    fn test_migration_v2_crud_entities() {
        let engine = StorageEngine::open_in_memory().expect("open memory db");
        engine
            .upsert_bookmark(&BookmarkEntry {
                id: "bm-1".into(),
                title: "Rust Docs".into(),
                url: "https://doc.rust-lang.org/".into(),
                category: "Dev".into(),
                created_at: 100,
            })
            .unwrap();
        assert_eq!(engine.get_bookmarks().unwrap().len(), 1);

        engine
            .upsert_note(&NoteEntry {
                id: "note-1".into(),
                title: "Architecture".into(),
                content: "Split View 50/50 validé".into(),
                url: Some("https://doc.rust-lang.org/".into()),
                updated_at: 200,
            })
            .unwrap();
        assert_eq!(engine.get_notes().unwrap().len(), 1);

        engine
            .save_session(&SessionRecord {
                id: "sess-1".into(),
                name: "Session Dev".into(),
                space_id: "default".into(),
                snapshot_json: "[]".into(),
                created_at: 300,
            })
            .unwrap();
        assert_eq!(engine.get_sessions().unwrap().len(), 1);

        engine.set_setting("theme", "light").unwrap();
        assert_eq!(
            engine.get_setting("theme").unwrap().as_deref(),
            Some("light")
        );

        engine
            .upsert_download(&DownloadRecord {
                id: "dl-v3".into(),
                url: "https://example.com/app.deb".into(),
                filename: "app.deb".into(),
                save_path: "/tmp/app.deb".into(),
                status: "En pause".into(),
                bytes_received: 512,
                total_bytes: 2048,
                started_at: 400,
            })
            .unwrap();
        let dls = engine.get_downloads().unwrap();
        assert_eq!(dls.len(), 1);
        assert_eq!(dls[0].total_bytes, 2048);
        engine.delete_download("dl-v3").unwrap();
        assert!(engine.get_downloads().unwrap().is_empty());

        engine
            .set_domain_permission(&DomainPermission {
                origin: "https://meet.example.com".into(),
                permission: "microphone".into(),
                decision: "deny".into(),
                updated_at: 500,
            })
            .unwrap();
        assert_eq!(engine.get_domain_permissions().unwrap().len(), 1);
        engine
            .delete_domain_permission("https://meet.example.com", "microphone")
            .unwrap();
        assert!(engine.get_domain_permissions().unwrap().is_empty());
    }
}
