pub mod local;

use crate::config::StorageConfig;
use std::path::Path;

/// Store a backup artifact based on the configuration.
///
/// Dispatches to the correct storage implementation (local, FTP, SFTP, …).
pub fn run(config: &StorageConfig, source_path: &Path) -> Result<(), String> {
    match config {
        StorageConfig::Local(local_config) => {
            let local = local::Local::new(local_config, source_path);
            local.perform()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::local::LocalConfig;

    #[test]
    fn run_dispatches_to_local_storage() {
        let source_directory = tempfile::tempdir().unwrap();
        let config = StorageConfig::Local(LocalConfig {
            path: "/tmp/rbak-test".to_string(),
        });

        let result = run(&config, source_directory.path());

        assert!(result.is_ok());
    }

    #[test]
    fn run_returns_local_storage_error() {
        let source_directory = tempfile::tempdir().unwrap();
        let config = StorageConfig::Local(LocalConfig {
            path: "".to_string(),
        });

        let result = run(&config, source_directory.path());

        assert!(result.is_err());
    }
}
