use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

use crate::error::Result;
use crate::storage::{ProfileRecord, StorageEngine};

/// Gestionnaire explicite de séparation des profils et du Mode Privé éphémère.
pub struct ProfileContext {
    pub record: ProfileRecord,
    pub root_dir: PathBuf,
    pub webview_data_dir: PathBuf,
    pub db_path: PathBuf,
    pub storage: StorageEngine,
}

impl ProfileContext {
    pub fn open_persistent(base_dir: &Path, id: &str, name: &str) -> Result<Self> {
        let root_dir = base_dir.join("profiles").join(id);
        let webview_data_dir = root_dir.join("webview_data");
        fs::create_dir_all(&webview_data_dir)?;
        let db_path = root_dir.join("orakle.sqlite");
        let storage = StorageEngine::open(&db_path)?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let record = ProfileRecord {
            id: id.to_string(),
            name: name.to_string(),
            data_dir: root_dir.to_string_lossy().to_string(),
            is_ephemeral: false,
            created_at: now,
        };
        storage.upsert_profile(&record)?;

        Ok(Self {
            record,
            root_dir,
            webview_data_dir,
            db_path,
            storage,
        })
    }

    /// Supprime explicitement l'intégralité des traces locales d'un profil (cookies, cache WebView, historique SQLite)
    pub fn purge_all_local_data(&self) -> Result<()> {
        self.storage.clear_history()?;
        if self.webview_data_dir.exists() {
            fs::remove_dir_all(&self.webview_data_dir)?;
            fs::create_dir_all(&self.webview_data_dir)?;
        }
        Ok(())
    }
}

/// Session Privée éphémère : aucune écriture dans la base du profil principal,
/// répertoire temporaire isolé détruit automatiquement à la fermeture (`Drop` ou `cleanup`).
pub struct PrivateSession {
    temp_dir: Option<TempDir>,
    pub temp_path: PathBuf,
    pub storage: StorageEngine,
}

impl PrivateSession {
    pub fn start() -> Result<Self> {
        let dir = tempfile::Builder::new()
            .prefix("orakle-private-")
            .tempdir()?;
        let temp_path = dir.path().to_path_buf();
        let storage = StorageEngine::open_in_memory()?;
        Ok(Self {
            temp_dir: Some(dir),
            temp_path,
            storage,
        })
    }

    pub fn cleanup(mut self) -> Result<()> {
        if let Some(dir) = self.temp_dir.take() {
            dir.close()?;
        }
        Ok(())
    }
}

impl Drop for PrivateSession {
    fn drop(&mut self) {
        if let Some(dir) = self.temp_dir.take() {
            let _ = dir.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::HistoryEntry;

    #[test]
    fn test_profile_isolation_and_private_cleanup() {
        let base = tempfile::tempdir().unwrap();
        let p_work = ProfileContext::open_persistent(base.path(), "work", "Travail").unwrap();
        let p_personal =
            ProfileContext::open_persistent(base.path(), "personal", "Personnel").unwrap();

        p_work
            .storage
            .insert_history_entry(&HistoryEntry {
                id: None,
                url: "https://internal.corp".into(),
                title: "Work".into(),
                visited_at: 100,
            })
            .unwrap();

        assert_eq!(p_work.storage.get_history(10).unwrap().len(), 1);
        assert_eq!(
            p_personal.storage.get_history(10).unwrap().len(),
            0,
            "Deux profils ne doivent pas partager leur historique"
        );

        let priv_sess = PrivateSession::start().unwrap();
        let priv_path = priv_sess.temp_path.clone();
        assert!(priv_path.exists());
        priv_sess
            .storage
            .insert_history_entry(&HistoryEntry {
                id: None,
                url: "https://secret.example".into(),
                title: "Privé".into(),
                visited_at: 200,
            })
            .unwrap();
        priv_sess.cleanup().unwrap();
        assert!(
            !priv_path.exists(),
            "Le stockage temporaire privé doit être détruit à la fermeture"
        );
        assert_eq!(p_work.storage.get_history(10).unwrap().len(), 1);
    }
}
