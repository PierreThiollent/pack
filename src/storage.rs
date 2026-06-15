pub mod ftp;
pub mod local;

use crate::storage::ftp::FtpConfig;
use crate::storage::local::LocalConfig;
use serde::Deserialize;
use std::path::Path;

/// Configuration for a storage — the `type` field determines which variant is used.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum StorageConfig {
    #[serde(rename = "local")]
    Local(LocalConfig),

    #[serde(rename = "ftp")]
    Ftp(FtpConfig),
}

/// Store a backup artifact based on the configuration.
///
/// Dispatches to the correct storage implementation (local, FTP, SFTP, …).
pub fn run(config: &StorageConfig, source_path: &Path) -> Result<(), String> {
    match config {
        StorageConfig::Local(local_config) => {
            let local = local::Local::new(local_config, source_path);
            local.perform()
        }
        StorageConfig::Ftp(ftp_config) => {
            let ftp = ftp::Ftp::new(ftp_config);
            ftp.perform()
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
            path: "/tmp/pack-test".to_string(),
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
