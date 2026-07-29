use crate::VaultError;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

/// App-global config, stored outside any vault (the "which vault" pointer can't
/// live inside the vault it names). Lives at `config_path()`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub vault_path: Option<PathBuf>,
    #[serde(default)]
    pub last_note: Option<String>,
    #[serde(default)]
    pub theme: String,
}

/// `<os-config-dir>/Silo/config.json` (e.g. `~/Library/Application Support/Silo/`).
pub fn config_path() -> PathBuf {
    directories::ProjectDirs::from("com", "silo", "Silo")
        .map(|d| d.config_dir().join("config.json"))
        .unwrap_or_else(|| PathBuf::from("silo-config.json"))
}

/// Load config; any missing/unreadable/malformed file yields defaults.
pub fn load_config(path: &Path) -> AppConfig {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Persist config, creating parent directories, via an atomic temp-write + rename.
pub fn save_config(path: &Path, cfg: &AppConfig) -> Result<(), VaultError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| VaultError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let json = serde_json::to_string_pretty(cfg).map_err(|e| VaultError::Serialize {
        path: path.to_path_buf(),
        msg: e.to_string(),
    })?;
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(dir).map_err(|source| VaultError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    tmp.write_all(json.as_bytes())
        .map_err(|source| VaultError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    tmp.persist(path).map_err(|e| VaultError::Io {
        path: path.to_path_buf(),
        source: e.error,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn config_roundtrips() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("sub/config.json");
        let cfg = AppConfig {
            vault_path: Some("/x/vault".into()),
            last_note: Some("01ARZ3NDEKTSV4RRFFQ69G5FAV".into()),
            theme: "dark".into(),
        };
        save_config(&p, &cfg).unwrap();
        assert_eq!(load_config(&p), cfg);
    }

    #[test]
    fn missing_config_is_default() {
        let dir = tempdir().unwrap();
        assert_eq!(
            load_config(&dir.path().join("nope.json")),
            AppConfig::default()
        );
    }

    #[test]
    fn malformed_config_is_default() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("bad.json");
        std::fs::write(&p, "{not json").unwrap();
        assert_eq!(load_config(&p), AppConfig::default());
    }
}
