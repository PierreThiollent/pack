pub mod ftp;
pub mod local;
pub mod sftp;

use crate::storage::ftp::FtpConfig;
use crate::storage::local::LocalConfig;
use crate::storage::sftp::SftpConfig;
use serde::Deserialize;
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::Path;
use std::time::Duration;

/// Configuration for a storage — the `type` field determines which variant is used.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum StorageConfig {
    #[serde(rename = "local")]
    Local(LocalConfig),

    #[serde(rename = "ftp")]
    Ftp(FtpConfig),

    #[serde(rename = "sftp")]
    Sftp(SftpConfig),
}

/// Store a backup artifact based on the configuration.
///
/// Dispatches to the correct storage implementation (local, FTP, SFTP, …).
pub fn run(config: &StorageConfig, source_path: &Path) -> Result<String, String> {
    match config {
        StorageConfig::Local(local_config) => {
            let local = local::Local::new(local_config, source_path);
            local.perform()
        }
        StorageConfig::Ftp(ftp_config) => {
            let ftp = ftp::Ftp::new(ftp_config, source_path);
            ftp.perform()?;
            artifact_file_key(source_path, "FTP")
        }
        StorageConfig::Sftp(sftp_config) => {
            let sftp = sftp::Sftp::new(sftp_config, source_path);
            sftp.perform()?;
            artifact_file_key(source_path, "SFTP")
        }
    }
}

pub fn delete(config: &StorageConfig, file_key: &str) -> Result<(), String> {
    match config {
        StorageConfig::Local(local_config) => local::delete(local_config, file_key),
        StorageConfig::Ftp(ftp_config) => ftp::delete(ftp_config, file_key),
        StorageConfig::Sftp(_) => Err("SFTP delete is not implemented yet".to_string()),
    }
}

impl StorageConfig {
    pub fn keep(&self) -> u32 {
        match self {
            StorageConfig::Local(config) => config.keep,
            StorageConfig::Ftp(config) => config.keep,
            StorageConfig::Sftp(config) => config.keep,
        }
    }
}

/// Return the artifact key used by storages and the cycler, based on the local file name.
pub(crate) fn artifact_file_key(source_path: &Path, storage_name: &str) -> Result<String, String> {
    source_path
        .file_name()
        .and_then(|file_name| file_name.to_str())
        .map(str::to_string)
        .ok_or_else(|| format!("{storage_name} source path has no file name: {source_path:?}"))
}

/// Format a remote endpoint as `host:port`.
pub(crate) fn remote_address(host: &str, port: u16) -> String {
    format!("{host}:{port}")
}

/// Resolve a remote `host:port` endpoint into a socket address.
pub(crate) fn socket_address(
    host: &str,
    port: u16,
    storage_name: &str,
) -> Result<SocketAddr, String> {
    remote_address(host, port)
        .to_socket_addrs()
        .map_err(|error| format!("Failed to resolve {storage_name} server address: {error}"))?
        .next()
        .ok_or_else(|| format!("Failed to resolve {storage_name} server address"))
}

/// Convert a timeout from seconds to a `Duration`.
pub(crate) fn timeout_duration(seconds: u64) -> Duration {
    Duration::from_secs(seconds)
}

/// Return each parent directory that must exist for a remote directory path.
///
/// Absolute paths stay absolute (`/backups/app`), while relative paths stay relative (`backups/app`).
pub(crate) fn remote_directories(path: &str) -> Vec<String> {
    let mut directories = Vec::new();
    let mut current = String::new();
    let is_absolute = path.starts_with('/');

    for segment in path.split('/').filter(|segment| !segment.is_empty()) {
        if current.is_empty() {
            if is_absolute {
                current.push('/');
            }
        } else {
            current.push('/');
        }

        current.push_str(segment);
        directories.push(current.clone());
    }

    directories
}

/// Build the final remote file path from a remote directory and a local artifact path.
pub(crate) fn remote_file_path(
    remote_directory: &str,
    source_path: &Path,
    storage_name: &str,
) -> Result<String, String> {
    let file_key = artifact_file_key(source_path, storage_name)?;
    remote_file_path_from_key(remote_directory, &file_key)
}

/// Build the final remote file path from a remote directory and a cycler file key.
pub(crate) fn remote_file_path_from_key(
    remote_directory: &str,
    file_key: &str,
) -> Result<String, String> {
    validate_file_key(file_key)?;

    let remote_directory = remote_directory.trim_end_matches('/');
    if remote_directory.is_empty() {
        Ok(format!("/{file_key}"))
    } else {
        Ok(format!("{remote_directory}/{file_key}"))
    }
}

/// Validate a cycler file key before joining it to a local or remote storage root.
///
/// The cycler state is local JSON and may be edited or corrupted, so storage deletion only accepts
/// simple artifact basenames and rejects path separators.
pub(crate) fn validate_file_key(file_key: &str) -> Result<(), String> {
    if file_key.trim().is_empty() || file_key.contains('/') || file_key.contains('\\') {
        return Err(format!("Invalid storage file key: {file_key}"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::ftp::FtpConfig;
    use crate::storage::local::LocalConfig;

    #[test]
    fn remote_directories_returns_no_directory_for_root() {
        assert!(remote_directories("/").is_empty());
    }

    #[test]
    fn remote_directories_returns_single_directory() {
        assert_eq!(remote_directories("/backups"), vec!["/backups"]);
    }

    #[test]
    fn remote_directories_returns_nested_directories_in_order() {
        assert_eq!(
            remote_directories("/backups/pack/prod"),
            vec!["/backups", "/backups/pack", "/backups/pack/prod"]
        );
    }

    #[test]
    fn remote_directories_ignores_repeated_and_trailing_slashes() {
        assert_eq!(
            remote_directories("//backups//pack/"),
            vec!["/backups", "/backups/pack"]
        );
    }

    #[test]
    fn remote_directories_preserves_relative_paths() {
        assert_eq!(
            remote_directories("pack/backups"),
            vec!["pack", "pack/backups"]
        );
    }

    #[test]
    fn remote_file_path_uses_configured_directory() {
        let source_path = Path::new("/tmp/my_app-20260616-120000.tar.gz");

        assert_eq!(
            remote_file_path("/backups", source_path, "FTP").unwrap(),
            "/backups/my_app-20260616-120000.tar.gz"
        );
    }

    #[test]
    fn remote_file_path_uses_root_directory() {
        let source_path = Path::new("/tmp/my_app-20260616-120000.tar.gz");

        assert_eq!(
            remote_file_path("/", source_path, "FTP").unwrap(),
            "/my_app-20260616-120000.tar.gz"
        );
    }

    #[test]
    fn remote_file_path_trims_trailing_slash() {
        let source_path = Path::new("/tmp/my_app-20260616-120000.tar.gz");

        assert_eq!(
            remote_file_path("/backups/", source_path, "FTP").unwrap(),
            "/backups/my_app-20260616-120000.tar.gz"
        );
    }

    #[test]
    fn remote_file_path_rejects_source_without_file_name() {
        let source_path = Path::new("/");

        assert!(remote_file_path("/backups", source_path, "FTP").is_err());
    }

    #[test]
    fn remote_file_path_from_key_uses_configured_directory() {
        assert_eq!(
            remote_file_path_from_key("/backups", "backup.tar.gz").unwrap(),
            "/backups/backup.tar.gz"
        );
    }

    #[test]
    fn remote_file_path_from_key_rejects_path_separators() {
        assert!(remote_file_path_from_key("/backups", "../backup.tar.gz").is_err());
        assert!(remote_file_path_from_key("/backups", "nested\\backup.tar.gz").is_err());
    }

    #[test]
    fn run_dispatches_to_local_storage() {
        let source_directory = tempfile::tempdir().unwrap();
        let config = StorageConfig::Local(LocalConfig {
            path: "/tmp/pack-test".to_string(),
            keep: 0,
        });

        let result = run(&config, source_directory.path());

        assert!(result.is_ok());
    }

    #[test]
    fn run_returns_local_storage_error() {
        let source_directory = tempfile::tempdir().unwrap();
        let config = StorageConfig::Local(LocalConfig {
            path: "".to_string(),
            keep: 0,
        });

        let result = run(&config, source_directory.path());

        assert!(result.is_err());
    }

    #[test]
    fn run_returns_local_file_key() {
        let source_directory = tempfile::tempdir().unwrap();
        let destination_directory = tempfile::tempdir().unwrap();
        let source_file = source_directory.path().join("backup.tar.gz");
        std::fs::write(&source_file, "backup").unwrap();
        let config = StorageConfig::Local(LocalConfig {
            path: destination_directory.path().to_string_lossy().into_owned(),
            keep: 2,
        });

        let file_key = run(&config, &source_file).unwrap();

        assert_eq!(file_key, "backup.tar.gz");
    }

    #[test]
    fn storage_config_keep_returns_local_keep() {
        let config = StorageConfig::Local(LocalConfig {
            path: "/tmp/pack-test".to_string(),
            keep: 7,
        });

        assert_eq!(config.keep(), 7);
    }

    #[test]
    fn storage_config_keep_returns_ftp_keep() {
        let config = StorageConfig::Ftp(FtpConfig {
            host: "ftp.example.com".to_string(),
            port: 21,
            timeout: 300,
            path: "/backups".to_string(),
            username: "user".to_string(),
            password: "secret".to_string(),
            explicit_tls: false,
            no_check_certificate: false,
            keep: 4,
        });

        assert_eq!(config.keep(), 4);
    }
}
