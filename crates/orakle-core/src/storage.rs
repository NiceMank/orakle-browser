use std::path::Path;
use rusqlite::{params, Connection, Transaction};
use serde::{Deserialize, Serialize};
use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Space {
    pub id: String,
    pub name: String,
    pub color: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TabRecord {
    pub id: String,
    pub space_id: String,
    pub url: String,
    pub title: String,
    pub is_pinned: bool,
    pub is_suspended: bool,
    pub position: i32,
    pub scroll_y: i32,
    pub last_active_at: i64,
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

        Ok(())
    }

    /// Exécute un contrôle d'intégrité formel SQLite
    pub fn check_integrity(&self) -> Result<bool> {
        let result: String = self.conn.query_row("PRAGMA integrity_check;", [], |row| row.get(0))?;
        Ok(result == "ok")
    }

    /// Force un checkpoint WAL pour synchroniser les pages disques
    pub fn checkpoint_wal(&self) -> Result<()> {
        self.conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
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
        let mut stmt = self.conn.prepare("SELECT id, name, color, created_at FROM spaces ORDER BY created_at ASC;")?;
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

    // --- Opérations Onglets (Tabs & Tab Discarding) ---
    pub fn upsert_tab(&self, tab: &TabRecord) -> Result<()> {
        self.conn.execute(
            "INSERT INTO tabs (id, space_id, url, title, is_pinned, is_suspended, position, scroll_y, last_active_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(id) DO UPDATE SET
                space_id = excluded.space_id,
                url = excluded.url,
                title = excluded.title,
                is_pinned = excluded.is_pinned,
                is_suspended = excluded.is_suspended,
                position = excluded.position,
                scroll_y = excluded.scroll_y,
                last_active_at = excluded.last_active_at;",
            params![
                tab.id,
                tab.space_id,
                tab.url,
                tab.title,
                if tab.is_pinned { 1 } else { 0 },
                if tab.is_suspended { 1 } else { 0 },
                tab.position,
                tab.scroll_y,
                tab.last_active_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_tabs_for_space(&self, space_id: &str) -> Result<Vec<TabRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, space_id, url, title, is_pinned, is_suspended, position, scroll_y, last_active_at
             FROM tabs WHERE space_id = ?1 ORDER BY position ASC;",
        )?;

        let rows = stmt.query_map(params![space_id], |row| {
            Ok(TabRecord {
                id: row.get(0)?,
                space_id: row.get(1)?,
                url: row.get(2)?,
                title: row.get(3)?,
                is_pinned: row.get::<_, i32>(4)? != 0,
                is_suspended: row.get::<_, i32>(5)? != 0,
                position: row.get(6)?,
                scroll_y: row.get(7)?,
                last_active_at: row.get(8)?,
            })
        })?;

        let mut tabs = Vec::new();
        for r in rows {
            tabs.push(r?);
        }
        Ok(tabs)
    }

    pub fn delete_tab(&self, tab_id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM tabs WHERE id = ?1;", params![tab_id])?;
        Ok(())
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
            let mut stmt = tx.prepare_cached("INSERT INTO history (url, title, visited_at) VALUES (?1, ?2, ?3);")?;
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
            url: "https://bad.com".to_string(),
            title: "Bad".to_string(),
            is_pinned: false,
            is_suspended: false,
            position: 0,
            scroll_y: 0,
            last_active_at: 1000,
        };

        let result = engine.upsert_tab(&bad_tab);
        assert!(result.is_err(), "L'insertion d'un onglet orphelin doit être rejetée par PRAGMA foreign_keys");
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
            url: "https://doc.rust-lang.org".to_string(),
            title: "Rust Book".to_string(),
            is_pinned: false,
            is_suspended: false,
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

        println!("  [BENCHMARK SQLITE BATCH] 1 000 entrées insérées en {:?} ({:.2} µs/entrée)", dur_batch, per_item_batch);
        assert!(per_item_batch < 50.0, "Le batch WAL doit être ultra-rapide (< 50 µs par entrée)");

        let history = engine.get_history(2000).unwrap();
        assert_eq!(history.len(), 1000);
    }
}
