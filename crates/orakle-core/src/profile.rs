use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::TempDir;

use crate::error::{OrakleError, Result};
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProfileSummary {
    pub id: String,
    pub name: String,
    pub created_at: i64,
    #[serde(default)]
    pub is_startup: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RegistryFile {
    startup_profile_id: String,
    profiles: Vec<ProfileSummary>,
}

/// Registre persistant des profils disponibles sur le poste utilisateur (`profiles.json`).
pub struct ProfileRegistry {
    base_dir: PathBuf,
    registry_path: PathBuf,
    startup_profile_id: String,
    profiles: Vec<ProfileSummary>,
}

fn now_epoch_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn sanitize_profile_slug(raw: &str) -> String {
    let mut slug = String::new();
    let mut prev_dash = false;
    for c in raw.trim().to_ascii_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
            prev_dash = false;
        } else if !prev_dash && !slug.is_empty() {
            slug.push('-');
            prev_dash = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() || slug == "default" {
        "profil".to_string()
    } else {
        slug.chars().take(32).collect()
    }
}

impl ProfileRegistry {
    pub fn load_or_init(base_dir: &Path) -> Result<Self> {
        fs::create_dir_all(base_dir)?;
        let registry_path = base_dir.join("profiles.json");
        if registry_path.exists() {
            let content = fs::read_to_string(&registry_path)?;
            if let Ok(parsed) = serde_json::from_str::<RegistryFile>(&content) {
                if !parsed.profiles.is_empty() {
                    let startup = if parsed
                        .profiles
                        .iter()
                        .any(|p| p.id == parsed.startup_profile_id)
                    {
                        parsed.startup_profile_id
                    } else {
                        parsed.profiles[0].id.clone()
                    };
                    let mut reg = Self {
                        base_dir: base_dir.to_path_buf(),
                        registry_path,
                        startup_profile_id: startup,
                        profiles: parsed.profiles,
                    };
                    reg.sync_startup_flags();
                    return Ok(reg);
                }
            }
        }

        let default_summary = ProfileSummary {
            id: "default".to_string(),
            name: "Profil Principal".to_string(),
            created_at: now_epoch_secs(),
            is_startup: true,
        };
        let _ = ProfileContext::open_persistent(base_dir, "default", "Profil Principal")?;
        let reg = Self {
            base_dir: base_dir.to_path_buf(),
            registry_path,
            startup_profile_id: "default".to_string(),
            profiles: vec![default_summary],
        };
        reg.save()?;
        Ok(reg)
    }

    fn sync_startup_flags(&mut self) {
        for p in &mut self.profiles {
            p.is_startup = p.id == self.startup_profile_id;
        }
    }

    pub fn save(&self) -> Result<()> {
        let payload = RegistryFile {
            startup_profile_id: self.startup_profile_id.clone(),
            profiles: self.profiles.clone(),
        };
        let json = serde_json::to_string_pretty(&payload)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let tmp_path = self.registry_path.with_extension("json.tmp");
        fs::write(&tmp_path, json)?;
        fs::rename(&tmp_path, &self.registry_path)?;
        Ok(())
    }

    pub fn startup_profile_id(&self) -> &str {
        &self.startup_profile_id
    }

    pub fn list_profiles(&self) -> Vec<ProfileSummary> {
        self.profiles.clone()
    }

    pub fn create_profile(&mut self, name: &str) -> Result<ProfileSummary> {
        let clean_name = name.trim();
        if clean_name.is_empty() {
            return Err(OrakleError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Le nom du profil ne peut pas être vide",
            )));
        }
        let base_slug = sanitize_profile_slug(clean_name);
        let mut candidate = base_slug.clone();
        let mut idx = 2usize;
        while self.profiles.iter().any(|p| p.id == candidate) {
            candidate = format!("{}-{}", base_slug, idx);
            idx += 1;
        }
        let _ = ProfileContext::open_persistent(&self.base_dir, &candidate, clean_name)?;
        let summary = ProfileSummary {
            id: candidate,
            name: clean_name.to_string(),
            created_at: now_epoch_secs(),
            is_startup: false,
        };
        self.profiles.push(summary.clone());
        self.sync_startup_flags();
        self.save()?;
        Ok(summary)
    }

    pub fn set_startup_profile(&mut self, id: &str) -> Result<()> {
        if !self.profiles.iter().any(|p| p.id == id) {
            return Err(OrakleError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Profil introuvable : {}", id),
            )));
        }
        self.startup_profile_id = id.to_string();
        self.sync_startup_flags();
        self.save()
    }

    pub fn delete_profile(&mut self, id: &str) -> Result<()> {
        if id == "default" || self.profiles.len() <= 1 {
            return Err(OrakleError::Io(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Impossible de supprimer le profil principal ou le dernier profil",
            )));
        }
        let before = self.profiles.len();
        self.profiles.retain(|p| p.id != id);
        if self.profiles.len() == before {
            return Err(OrakleError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Profil introuvable : {}", id),
            )));
        }
        if self.startup_profile_id == id {
            self.startup_profile_id = "default".to_string();
        }
        self.sync_startup_flags();
        self.save()?;
        let profile_dir = self.base_dir.join("profiles").join(id);
        if profile_dir.exists() {
            fs::remove_dir_all(&profile_dir)?;
        }
        Ok(())
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

    #[test]
    fn test_profile_registry_lifecycle() {
        let base = tempfile::tempdir().unwrap();
        let mut reg = ProfileRegistry::load_or_init(base.path()).unwrap();
        assert_eq!(reg.startup_profile_id(), "default");
        assert_eq!(reg.list_profiles().len(), 1);

        let work = reg.create_profile("Travail Dev").unwrap();
        assert_eq!(work.id, "travail-dev");
        assert!(base.path().join("profiles").join("travail-dev").exists());

        reg.set_startup_profile(&work.id).unwrap();
        assert_eq!(reg.startup_profile_id(), "travail-dev");

        // Rechargement depuis le disque
        let mut reg2 = ProfileRegistry::load_or_init(base.path()).unwrap();
        assert_eq!(reg2.startup_profile_id(), "travail-dev");
        assert_eq!(reg2.list_profiles().len(), 2);

        // Interdiction de supprimer le profil principal
        assert!(reg2.delete_profile("default").is_err());

        // Suppression du profil secondaire et nettoyage disque
        reg2.delete_profile(&work.id).unwrap();
        assert_eq!(reg2.startup_profile_id(), "default");
        assert_eq!(reg2.list_profiles().len(), 1);
        assert!(!base.path().join("profiles").join("travail-dev").exists());
    }
}
