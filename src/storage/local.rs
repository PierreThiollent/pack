use crate::logging::{LogTag, tag};
use crate::paths;
use crate::storage::{StorageRunResult, artifact_file_key, delete_old_backups, validate_file_key};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use tracing::info;

/// Configuration specific to local storage.
#[derive(Debug, Deserialize)]
pub struct LocalConfig {
    pub path: String,

    #[serde(default)]
    pub keep: u32,
}

/// Local filesystem storage.
pub struct Local<'a> {
    config: &'a LocalConfig,
    source_path: &'a Path,
}

impl<'a> Local<'a> {
    /// Create a new local storage handler.
    ///
    /// * `config` — parsed local storage config from the YAML file
    /// * `source_path` — file or directory that will be copied later
    pub fn new(config: &'a LocalConfig, source_path: &'a Path) -> Self {
        Self {
            config,
            source_path,
        }
    }

    /// Store the source path in the configured local destination.
    pub fn perform(&self, delete_after_upload: &[String]) -> Result<StorageRunResult, String> {
        if self.config.path.trim().is_empty() {
            return Err("Local storage path cannot be empty".to_string());
        }

        if !self.source_path.exists() {
            return Err(format!(
                "Local storage source path does not exist: {:?}",
                self.source_path
            ));
        }

        let destination_path = self.destination_path()?;

        if self.source_path.is_dir() {
            copy_directory(self.source_path, &destination_path)?;
        } else {
            copy_file(self.source_path, &destination_path)?;
        }

        info!(
            pack_tag = %tag(LogTag::Local),
            "Store succeeded: {}",
            destination_path.display()
        );

        let deleted_file_keys = delete_old_backups(delete_after_upload, |file_key| {
            delete_file(self.config, file_key)
        });

        Ok(StorageRunResult { deleted_file_keys })
    }

    /// Build the final destination path by joining configured root and source name.
    fn destination_path(&self) -> Result<PathBuf, String> {
        Ok(root_directory(&self.config.path)
            .join(artifact_file_key(self.source_path, "Local storage")?))
    }
}

pub(crate) fn delete_file(config: &LocalConfig, file_key: &str) -> Result<(), String> {
    validate_file_key(file_key)?;

    let path = root_directory(&config.path).join(file_key);
    std::fs::remove_file(&path)
        .map_err(|error| format!("Failed to delete local storage file {path:?}: {error}"))
}

/// Return the configured local storage root with a leading `~` expanded.
fn root_directory(path: &str) -> PathBuf {
    PathBuf::from(paths::expand_tilde(path))
}

/// Copy one file, creating its parent destination directory first.
fn copy_file(source_path: &Path, destination_path: &Path) -> Result<(), String> {
    if let Some(parent_directory) = destination_path.parent() {
        std::fs::create_dir_all(parent_directory).map_err(|error| {
            format!("Failed to create local storage directory {parent_directory:?}: {error}")
        })?;
    }

    std::fs::copy(source_path, destination_path).map_err(|error| {
        format!(
            "Failed to copy local storage file {source_path:?} to {destination_path:?}: {error}"
        )
    })?;

    Ok(())
}

/// Recursively copy a directory and all of its children.
fn copy_directory(source_path: &Path, destination_path: &Path) -> Result<(), String> {
    std::fs::create_dir_all(destination_path).map_err(|error| {
        format!("Failed to create local storage directory {destination_path:?}: {error}")
    })?;

    for entry in std::fs::read_dir(source_path)
        .map_err(|error| format!("Failed to read local storage source {source_path:?}: {error}"))?
    {
        let entry = entry.map_err(|error| {
            format!("Failed to read local storage source entry in {source_path:?}: {error}")
        })?;
        let child_source_path = entry.path();
        let child_destination_path = destination_path.join(entry.file_name());

        if child_source_path.is_dir() {
            copy_directory(&child_source_path, &child_destination_path)?;
        } else {
            copy_file(&child_source_path, &child_destination_path)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(path: &str) -> LocalConfig {
        LocalConfig {
            path: path.to_string(),
            keep: 0,
        }
    }

    #[test]
    fn perform_copies_existing_source_directory() {
        let source_directory = tempfile::tempdir().unwrap();
        let destination_directory = tempfile::tempdir().unwrap();
        let source_file = source_directory.path().join("dump.sql");
        std::fs::write(&source_file, "dump content").unwrap();
        let config = make_config(&destination_directory.path().to_string_lossy());
        let local = Local::new(&config, source_directory.path());

        let result = local.perform(&[]);

        assert!(result.is_ok());
        let copied_file = destination_directory
            .path()
            .join(source_directory.path().file_name().unwrap())
            .join("dump.sql");
        assert_eq!(
            std::fs::read_to_string(copied_file).unwrap(),
            "dump content"
        );
    }

    #[test]
    fn perform_copies_nested_source_directory() {
        let source_directory = tempfile::tempdir().unwrap();
        let destination_directory = tempfile::tempdir().unwrap();
        let nested_directory = source_directory.path().join("nested");
        std::fs::create_dir_all(&nested_directory).unwrap();
        std::fs::write(nested_directory.join("dump.sql"), "nested dump").unwrap();
        let config = make_config(&destination_directory.path().to_string_lossy());
        let local = Local::new(&config, source_directory.path());

        let result = local.perform(&[]);

        assert!(result.is_ok());
        let copied_file = destination_directory
            .path()
            .join(source_directory.path().file_name().unwrap())
            .join("nested")
            .join("dump.sql");
        assert_eq!(std::fs::read_to_string(copied_file).unwrap(), "nested dump");
    }

    #[test]
    fn perform_copies_existing_source_file() {
        let source_directory = tempfile::tempdir().unwrap();
        let destination_directory = tempfile::tempdir().unwrap();
        let source_file = source_directory.path().join("dump.sql");
        std::fs::write(&source_file, "dump content").unwrap();
        let config = make_config(&destination_directory.path().to_string_lossy());
        let local = Local::new(&config, &source_file);

        let result = local.perform(&[]);

        assert!(result.is_ok());
        let copied_file = destination_directory.path().join("dump.sql");
        assert_eq!(
            std::fs::read_to_string(copied_file).unwrap(),
            "dump content"
        );
    }

    #[test]
    fn perform_rejects_empty_destination_path() {
        let source_directory = tempfile::tempdir().unwrap();
        let config = make_config("");
        let local = Local::new(&config, source_directory.path());

        let result = local.perform(&[]);

        assert!(result.is_err());
    }

    #[test]
    fn perform_rejects_missing_source_path() {
        let config = make_config("/tmp/pack-test");
        let local = Local::new(&config, Path::new("/path/that/does/not/exist"));

        let result = local.perform(&[]);

        assert!(result.is_err());
    }

    #[test]
    fn perform_returns_file_key() {
        let source_directory = tempfile::tempdir().unwrap();
        let destination_directory = tempfile::tempdir().unwrap();
        let source_file = source_directory.path().join("backup.tar.gz");
        std::fs::write(&source_file, "backup").unwrap();
        let config = make_config(&destination_directory.path().to_string_lossy());
        let local = Local::new(&config, &source_file);

        let result = local.perform(&[]).unwrap();

        assert!(result.deleted_file_keys.is_empty());
    }

    #[test]
    fn perform_deletes_old_backups_after_copy() {
        let source_directory = tempfile::tempdir().unwrap();
        let destination_directory = tempfile::tempdir().unwrap();
        let source_file = source_directory.path().join("backup.tar.gz");
        let old_backup_file = destination_directory.path().join("old.tar.gz");
        std::fs::write(&source_file, "backup").unwrap();
        std::fs::write(&old_backup_file, "old backup").unwrap();
        let config = make_config(&destination_directory.path().to_string_lossy());
        let local = Local::new(&config, &source_file);

        let result = local.perform(&["old.tar.gz".to_string()]).unwrap();

        assert_eq!(result.deleted_file_keys, vec!["old.tar.gz"]);
        assert!(!old_backup_file.exists());
    }

    #[test]
    fn delete_removes_local_file() {
        let destination_directory = tempfile::tempdir().unwrap();
        let backup_file = destination_directory.path().join("backup.tar.gz");
        std::fs::write(&backup_file, "backup").unwrap();
        let config = make_config(&destination_directory.path().to_string_lossy());

        delete_file(&config, "backup.tar.gz").unwrap();

        assert!(!backup_file.exists());
    }

    #[test]
    fn delete_rejects_unsafe_file_key() {
        let destination_directory = tempfile::tempdir().unwrap();
        let config = make_config(&destination_directory.path().to_string_lossy());

        let result = delete_file(&config, "../backup.tar.gz");

        assert!(result.is_err());
    }
}
