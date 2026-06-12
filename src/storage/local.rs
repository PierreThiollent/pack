use crate::paths;
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Configuration specific to local storage.
#[derive(Debug, Deserialize)]
pub struct LocalConfig {
    pub path: String,
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
    pub fn perform(&self) -> Result<(), String> {
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

        println!("    Stored locally: {}", destination_path.display());
        Ok(())
    }

    fn destination_path(&self) -> Result<PathBuf, String> {
        let root_directory = PathBuf::from(paths::expand_tilde(&self.config.path));
        let source_name = self.source_path.file_name().ok_or_else(|| {
            format!(
                "Local storage source path has no file name: {:?}",
                self.source_path
            )
        })?;

        Ok(root_directory.join(source_name))
    }
}

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

        let result = local.perform();

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

        let result = local.perform();

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

        let result = local.perform();

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

        let result = local.perform();

        assert!(result.is_err());
    }

    #[test]
    fn perform_rejects_missing_source_path() {
        let config = make_config("/tmp/rbak-test");
        let local = Local::new(&config, Path::new("/path/that/does/not/exist"));

        let result = local.perform();

        assert!(result.is_err());
    }
}
