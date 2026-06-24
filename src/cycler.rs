use crate::logging::{LogTag, tag};
use crate::paths;
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::warn;

/// One stored backup tracked by the cycler.
#[derive(Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Package {
    pub file_key: String,
    pub created_at: String,
}

/// Local retention state for one model/storage pair.
#[derive(Debug, Default)]
pub struct Cycler {
    packages: Vec<Package>,
}

/// Build the cycler state path for one model/storage pair.
pub(crate) fn state_path(root_directory: &Path, model_name: &str, storage_name: &str) -> PathBuf {
    root_directory
        .join("cycler")
        .join(format!("{model_name}_{storage_name}.json"))
}

/// Build the default cycler state path under `~/.pack`.
pub(crate) fn default_state_path(model_name: &str, storage_name: &str) -> PathBuf {
    state_path(
        &PathBuf::from(paths::home_dir()).join(".pack"),
        model_name,
        storage_name,
    )
}

impl Cycler {
    /// Create an empty cycler state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load cycler state from a JSON file.
    ///
    /// A missing file means this model/storage pair has no retention state yet.
    pub fn load_from_path(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::new());
        }

        let content = std::fs::read_to_string(path)
            .map_err(|error| format!("Failed to read cycler state {path:?}: {error}"))?;
        let packages = match serde_json::from_str(&content) {
            Ok(packages) => packages,
            Err(error) => {
                warn!(
                    pack_tag = %tag(LogTag::Cycler),
                    "Failed to parse cycler state {path:?}: {error}. Starting from an empty state"
                );
                Vec::new()
            }
        };

        Ok(Self { packages })
    }

    /// Add a newly stored backup to the end of the state.
    pub fn add(&mut self, file_key: &str) {
        self.packages.push(Package {
            file_key: file_key.to_string(),
            created_at: Local::now().to_rfc3339(),
        });
    }

    /// Add a backup, apply retention, and return file keys that should be deleted.
    pub fn record_and_prune(&mut self, file_key: &str, keep: u32) -> Vec<String> {
        self.add(file_key);
        self.prune(keep)
    }

    /// Keep the newest `keep` packages and return removed file keys.
    pub fn prune(&mut self, keep: u32) -> Vec<String> {
        if keep == 0 {
            return Vec::new();
        }

        let keep = keep as usize;
        let remove_count = self.packages.len().saturating_sub(keep);
        self.packages
            .drain(..remove_count)
            .map(|package| package.file_key)
            .collect()
    }

    /// Load state, record a new package, apply retention, save state, and return file keys to delete.
    pub fn record_and_prune_path(
        path: &Path,
        file_key: &str,
        keep: u32,
    ) -> Result<Vec<String>, String> {
        let mut cycler = Self::load_from_path(path)?;
        let removed_file_keys = cycler.record_and_prune(file_key, keep);
        cycler.save_to_path(path)?;
        Ok(removed_file_keys)
    }

    /// Save cycler state as JSON.
    pub fn save_to_path(&self, path: &Path) -> Result<(), String> {
        if let Some(parent_directory) = path.parent() {
            std::fs::create_dir_all(parent_directory).map_err(|error| {
                format!("Failed to create cycler directory {parent_directory:?}: {error}")
            })?;
        }

        let content = serde_json::to_string_pretty(&self.packages)
            .map_err(|error| format!("Failed to serialize cycler state {path:?}: {error}"))?;
        std::fs::write(path, content)
            .map_err(|error| format!("Failed to write cycler state {path:?}: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_appends_package() {
        let mut cycler = Cycler::new();

        cycler.add("my_app-20260623-120000.tar.gz");

        assert_eq!(cycler.packages.len(), 1);
        assert_eq!(cycler.packages[0].file_key, "my_app-20260623-120000.tar.gz");
        assert!(!cycler.packages[0].created_at.is_empty());
    }

    #[test]
    fn package_serializes_to_json() {
        let package = Package {
            file_key: "my_app-20260623-120000.tar.gz".to_string(),
            created_at: "2026-06-23T12:00:00+02:00".to_string(),
        };

        let json = serde_json::to_string(&package).unwrap();

        assert_eq!(
            json,
            r#"{"file_key":"my_app-20260623-120000.tar.gz","created_at":"2026-06-23T12:00:00+02:00"}"#
        );
    }

    #[test]
    fn package_deserializes_from_json() {
        let json = r#"{"file_key":"my_app-20260623-120000.tar.gz","created_at":"2026-06-23T12:00:00+02:00"}"#;

        let package: Package = serde_json::from_str(json).unwrap();

        assert_eq!(package.file_key, "my_app-20260623-120000.tar.gz");
        assert_eq!(package.created_at, "2026-06-23T12:00:00+02:00");
    }

    #[test]
    fn load_from_path_returns_empty_cycler_when_file_is_missing() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("missing.json");

        let cycler = Cycler::load_from_path(&path).unwrap();

        assert!(cycler.packages.is_empty());
    }

    #[test]
    fn save_to_path_writes_json_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("cycler").join("my_app_local.json");
        let mut cycler = Cycler::new();
        cycler.add("my_app-20260623-120000.tar.gz");

        cycler.save_to_path(&path).unwrap();

        let content = std::fs::read_to_string(path).unwrap();
        assert!(content.contains("my_app-20260623-120000.tar.gz"));
    }

    #[test]
    fn save_and_load_packages() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("my_app_local.json");
        let mut cycler = Cycler::new();
        cycler.add("my_app-20260623-120000.tar.gz");

        cycler.save_to_path(&path).unwrap();
        let loaded_cycler = Cycler::load_from_path(&path).unwrap();

        assert_eq!(loaded_cycler.packages.len(), 1);
        assert_eq!(
            loaded_cycler.packages[0].file_key,
            "my_app-20260623-120000.tar.gz"
        );
    }

    #[test]
    fn load_from_path_returns_empty_cycler_when_json_is_invalid() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("broken.json");
        std::fs::write(&path, "not json").unwrap();

        let cycler = Cycler::load_from_path(&path).unwrap();

        assert!(cycler.packages.is_empty());
    }

    #[test]
    fn state_path_uses_model_and_storage_names() {
        let root_directory = Path::new("/home/pierre/.pack");

        let path = state_path(root_directory, "my_app", "local");

        assert_eq!(
            path,
            PathBuf::from("/home/pierre/.pack/cycler/my_app_local.json")
        );
    }

    #[test]
    fn default_state_path_uses_pack_home_directory() {
        let home = std::env::var("HOME").unwrap();

        let path = default_state_path("my_app", "sftp");

        assert_eq!(
            path,
            PathBuf::from(format!("{home}/.pack/cycler/my_app_sftp.json"))
        );
    }

    #[test]
    fn prune_keeps_all_packages_when_keep_is_zero() {
        let mut cycler = Cycler::new();
        cycler.add("old.tar.gz");
        cycler.add("new.tar.gz");

        let removed = cycler.prune(0);

        assert!(removed.is_empty());
        assert_eq!(cycler.packages.len(), 2);
    }

    #[test]
    fn prune_keeps_newest_packages() {
        let mut cycler = Cycler::new();
        cycler.add("oldest.tar.gz");
        cycler.add("middle.tar.gz");
        cycler.add("newest.tar.gz");

        let removed = cycler.prune(2);

        assert_eq!(removed, vec!["oldest.tar.gz"]);
        assert_eq!(cycler.packages.len(), 2);
        assert_eq!(cycler.packages[0].file_key, "middle.tar.gz");
        assert_eq!(cycler.packages[1].file_key, "newest.tar.gz");
    }

    #[test]
    fn prune_removes_multiple_old_packages() {
        let mut cycler = Cycler::new();
        cycler.add("one.tar.gz");
        cycler.add("two.tar.gz");
        cycler.add("three.tar.gz");
        cycler.add("four.tar.gz");

        let removed = cycler.prune(1);

        assert_eq!(removed, vec!["one.tar.gz", "two.tar.gz", "three.tar.gz"]);
        assert_eq!(cycler.packages.len(), 1);
        assert_eq!(cycler.packages[0].file_key, "four.tar.gz");
    }

    #[test]
    fn record_and_prune_adds_new_package_before_pruning() {
        let mut cycler = Cycler::new();
        cycler.add("old.tar.gz");
        cycler.add("current.tar.gz");

        let removed = cycler.record_and_prune("new.tar.gz", 2);

        assert_eq!(removed, vec!["old.tar.gz"]);
        assert_eq!(cycler.packages.len(), 2);
        assert_eq!(cycler.packages[0].file_key, "current.tar.gz");
        assert_eq!(cycler.packages[1].file_key, "new.tar.gz");
    }

    #[test]
    fn record_and_prune_path_saves_updated_state() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("my_app_local.json");

        let removed = Cycler::record_and_prune_path(&path, "first.tar.gz", 2).unwrap();
        assert!(removed.is_empty());

        let removed = Cycler::record_and_prune_path(&path, "second.tar.gz", 2).unwrap();
        assert!(removed.is_empty());

        let removed = Cycler::record_and_prune_path(&path, "third.tar.gz", 2).unwrap();

        assert_eq!(removed, vec!["first.tar.gz"]);
        let loaded_cycler = Cycler::load_from_path(&path).unwrap();
        assert_eq!(loaded_cycler.packages.len(), 2);
        assert_eq!(loaded_cycler.packages[0].file_key, "second.tar.gz");
        assert_eq!(loaded_cycler.packages[1].file_key, "third.tar.gz");
    }

    #[test]
    fn record_and_prune_path_keeps_all_when_keep_is_zero() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("my_app_local.json");

        Cycler::record_and_prune_path(&path, "first.tar.gz", 0).unwrap();
        let removed = Cycler::record_and_prune_path(&path, "second.tar.gz", 0).unwrap();

        assert!(removed.is_empty());
        let loaded_cycler = Cycler::load_from_path(&path).unwrap();
        assert_eq!(loaded_cycler.packages.len(), 2);
    }
}
