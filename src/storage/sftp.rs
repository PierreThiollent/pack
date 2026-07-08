use crate::logging::{LogTag, tag};
use crate::storage::ssh::SshConnectionConfig;
use crate::storage::{
    Storage, StorageRunResult, delete_old_backups, remote_directories, remote_file_path,
    remote_file_path_from_key,
};
use serde::Deserialize;
use ssh2::{Session, Sftp as Ssh2Sftp};
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::time::{Duration, Instant};
use tracing::info;

// Keep SFTP writes large enough to avoid many small libssh2 write calls.
const SFTP_UPLOAD_BUFFER_SIZE: usize = 4 * 1024 * 1024;

/// Configuration specific to SFTP storage.
#[derive(Debug, Deserialize)]
pub struct SftpConfig {
    pub host: String,

    #[serde(default = "default_port")]
    pub port: u16,

    #[serde(default = "default_timeout")]
    pub timeout: u64,

    #[serde(default = "default_path")]
    pub path: String,

    pub username: String,

    #[serde(default)]
    pub password: Option<String>,

    #[serde(default)]
    pub private_key: Option<String>,

    #[serde(default)]
    pub passphrase: Option<String>,

    #[serde(default)]
    pub keep: u32,
}

/// SFTP storage handler for one artifact upload.
pub struct Sftp<'a> {
    config: &'a SftpConfig,
    source_path: &'a Path,
}

impl<'a> Sftp<'a> {
    /// Create a new SFTP storage handler.
    pub fn new(config: &'a SftpConfig, source_path: &'a Path) -> Self {
        Self {
            config,
            source_path,
        }
    }

    /// Connect, authenticate, create remote directories and upload the artifact.
    pub fn perform(&self, delete_after_upload: &[String]) -> Result<StorageRunResult, String> {
        let ssh_config = self.ssh_config();
        crate::storage::ssh::validate_config(&ssh_config)?;

        let mut session = crate::storage::ssh::connect(&ssh_config)?;
        crate::storage::ssh::authenticate(&ssh_config, &mut session)?;
        let sftp_session = self.open_sftp_subsystem(&session)?;
        self.ensure_remote_directory(&sftp_session)?;
        let remote_path = self.remote_path()?;
        self.upload(&sftp_session, &remote_path)?;
        info!(pack_tag = %tag(LogTag::Sftp), "Store succeeded: {remote_path}");

        let deleted_file_keys = delete_old_backups(delete_after_upload, |file_key| {
            let remote_path = remote_file_path_from_key(&self.config.path, file_key)?;
            delete_with_session(&sftp_session, &remote_path)
        });

        Ok(StorageRunResult { deleted_file_keys })
    }

    /// Create the configured remote directory and its parents when missing.
    fn ensure_remote_directory(&self, sftp_session: &Ssh2Sftp) -> Result<(), String> {
        for directory in remote_directories(&self.config.path) {
            let directory_path = Path::new(&directory);
            if sftp_session.stat(directory_path).is_ok() {
                continue;
            }

            info!(
                pack_tag = %tag(LogTag::Sftp),
                "Creating remote directory: {directory}"
            );
            sftp_session
                .mkdir(directory_path, 0o755)
                .map_err(|error| format!("Failed to create SFTP directory {directory}: {error}"))?;
        }

        Ok(())
    }

    /// Upload the local artifact to the final remote path.
    fn upload(&self, sftp_session: &Ssh2Sftp, remote_path: &str) -> Result<(), String> {
        let mut source_file = File::open(self.source_path).map_err(|error| {
            format!(
                "Failed to open SFTP source file {:?}: {error}",
                self.source_path
            )
        })?;
        let source_size = source_file.metadata().map(|metadata| metadata.len()).ok();

        let mut remote_file = sftp_session
            .create(Path::new(remote_path))
            .map_err(|error| format!("Failed to create SFTP remote file {remote_path}: {error}"))?;

        info!(
            pack_tag = %tag(LogTag::Sftp),
            "Uploading backup: {remote_path}"
        );
        let started_at = Instant::now();
        let bytes_uploaded = copy_with_buffer(&mut source_file, &mut remote_file)
            .map_err(|error| format!("Failed to upload SFTP file to {remote_path}: {error}"))?;
        remote_file
            .flush()
            .map_err(|error| format!("Failed to flush SFTP remote file {remote_path}: {error}"))?;

        log_upload_duration(bytes_uploaded, source_size, started_at.elapsed());

        Ok(())
    }

    /// Open the SFTP subsystem over the authenticated SSH session.
    fn open_sftp_subsystem(&self, session: &Session) -> Result<Ssh2Sftp, String> {
        let sftp_session = session
            .sftp()
            .map_err(|error| format!("Failed to open SFTP subsystem: {error}"))?;

        info!(pack_tag = %tag(LogTag::Sftp), "SFTP session opened");
        Ok(sftp_session)
    }

    /// Build the shared SSH connection settings used by the SFTP backend.
    fn ssh_config(&self) -> SshConnectionConfig<'_> {
        SshConnectionConfig {
            storage_name: "SFTP",
            log_tag: LogTag::Sftp,
            host: &self.config.host,
            port: self.config.port,
            timeout: self.config.timeout,
            username: &self.config.username,
            password: self.config.password.as_deref(),
            private_key: self.config.private_key.as_deref(),
            passphrase: self.config.passphrase.as_deref(),
        }
    }

    /// Build the final remote path from configured directory and artifact name.
    fn remote_path(&self) -> Result<String, String> {
        remote_file_path(&self.config.path, self.source_path, "SFTP")
    }
}

impl Storage for SftpConfig {
    fn keep(&self) -> u32 {
        self.keep
    }

    fn perform(
        &self,
        source_path: &Path,
        delete_after_upload: &[String],
    ) -> Result<StorageRunResult, String> {
        Sftp::new(self, source_path).perform(delete_after_upload)
    }
}

fn delete_with_session(sftp_session: &Ssh2Sftp, remote_path: &str) -> Result<(), String> {
    sftp_session
        .unlink(Path::new(remote_path))
        .map_err(|error| format!("Failed to delete SFTP file {remote_path}: {error}"))?;

    Ok(())
}

/// Copy bytes with a large buffer to reduce costly libssh2 SFTP write calls.
fn copy_with_buffer<R: Read, W: Write>(reader: &mut R, writer: &mut W) -> io::Result<u64> {
    let mut buffer = vec![0; SFTP_UPLOAD_BUFFER_SIZE];
    let mut bytes_copied = 0;

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            return Ok(bytes_copied);
        }

        writer.write_all(&buffer[..bytes_read])?;
        bytes_copied += bytes_read as u64;
    }
}

/// Log uploaded size and throughput using decimal MB, like most FTP clients.
fn log_upload_duration(bytes_uploaded: u64, source_size: Option<u64>, duration: Duration) {
    let seconds = duration.as_secs_f64();
    let megabytes_uploaded = bytes_uploaded as f64 / 1_000_000.0;

    if seconds > 0.0 {
        info!(
            pack_tag = %tag(LogTag::Sftp),
            "Uploaded {:.2} MB in {:.2}s ({:.2} MB/s)",
            megabytes_uploaded,
            seconds,
            megabytes_uploaded / seconds
        );
    } else {
        info!(
            pack_tag = %tag(LogTag::Sftp),
            "Uploaded {:.2} MB",
            megabytes_uploaded
        );
    }

    if let Some(expected_size) = source_size
        && expected_size != bytes_uploaded
    {
        info!(
            pack_tag = %tag(LogTag::Sftp),
            "Local file size changed during upload: expected {} bytes, uploaded {} bytes",
            expected_size,
            bytes_uploaded
        );
    }
}

fn default_port() -> u16 {
    22
}

fn default_timeout() -> u64 {
    300
}

fn default_path() -> String {
    "/".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_config() -> SftpConfig {
        SftpConfig {
            host: "sftp.example.com".to_string(),
            port: 22,
            timeout: 300,
            path: "/backups".to_string(),
            username: "user".to_string(),
            password: Some("secret".to_string()),
            private_key: None,
            passphrase: None,
            keep: 0,
        }
    }

    #[test]
    fn remote_address_uses_host_and_port() {
        let mut config = valid_config();
        config.host = "sftp.example.com".to_string();
        config.port = 2222;
        let source_path = Path::new("backup.tar.gz");
        let sftp = Sftp::new(&config, source_path);

        assert_eq!(
            crate::storage::remote_address(&sftp.config.host, sftp.config.port),
            "sftp.example.com:2222"
        );
    }

    #[test]
    fn ssh_config_maps_sftp_fields() {
        let mut config = valid_config();
        config.host = "example.com".to_string();
        config.port = 2222;
        config.timeout = 42;
        config.username = "deploy".to_string();
        config.password = Some("secret".to_string());
        config.private_key = Some("~/.ssh/id_ed25519".to_string());
        config.passphrase = Some("key-passphrase".to_string());
        let sftp = Sftp::new(&config, Path::new("backup.tar.gz"));

        let ssh_config = sftp.ssh_config();

        assert_eq!(ssh_config.storage_name, "SFTP");
        assert_eq!(ssh_config.log_tag, LogTag::Sftp);
        assert_eq!(ssh_config.host, "example.com");
        assert_eq!(ssh_config.port, 2222);
        assert_eq!(ssh_config.timeout, 42);
        assert_eq!(ssh_config.username, "deploy");
        assert_eq!(ssh_config.password, Some("secret"));
        assert_eq!(ssh_config.private_key, Some("~/.ssh/id_ed25519"));
        assert_eq!(ssh_config.passphrase, Some("key-passphrase"));
    }

    #[test]
    fn remote_path_uses_shared_remote_file_path_helper() {
        let config = valid_config();
        let source_path = Path::new("/tmp/my_app-20260616-120000.tar.gz");
        let sftp = Sftp::new(&config, source_path);

        assert_eq!(
            sftp.remote_path().unwrap(),
            "/backups/my_app-20260616-120000.tar.gz"
        );
    }

    #[test]
    fn copy_with_buffer_copies_all_bytes() {
        let input = b"hello sftp upload";
        let mut reader = &input[..];
        let mut writer = Vec::new();

        let bytes_copied = copy_with_buffer(&mut reader, &mut writer).unwrap();

        assert_eq!(bytes_copied, input.len() as u64);
        assert_eq!(writer, input);
    }

    #[test]
    fn copy_with_buffer_handles_empty_reader() {
        let input = b"";
        let mut reader = &input[..];
        let mut writer = Vec::new();

        let bytes_copied = copy_with_buffer(&mut reader, &mut writer).unwrap();

        assert_eq!(bytes_copied, 0);
        assert!(writer.is_empty());
    }
}
