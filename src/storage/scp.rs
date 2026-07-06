use crate::logging::{LogTag, tag};
use crate::storage::ssh::SshConnectionConfig;
use crate::storage::{
    Storage, StorageRunResult, delete_old_backups, remote_directories, remote_file_path,
    remote_file_path_from_key,
};
use serde::Deserialize;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::time::{Duration, Instant};
use tracing::info;

// Keep SCP writes large enough to avoid many small libssh2 write calls.
const SCP_UPLOAD_BUFFER_SIZE: usize = 4 * 1024 * 1024;

/// Configuration specific to SCP storage.
#[derive(Debug, Deserialize)]
pub struct ScpConfig {
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

/// SCP storage handler for one artifact upload.
pub struct Scp<'a> {
    config: &'a ScpConfig,
    source_path: &'a Path,
}

impl<'a> Scp<'a> {
    /// Create a new SCP storage handler.
    pub fn new(config: &'a ScpConfig, source_path: &'a Path) -> Self {
        Self {
            config,
            source_path,
        }
    }

    /// Connect and authenticate over SSH before the upload implementation is added.
    pub fn perform(&self, delete_after_upload: &[String]) -> Result<StorageRunResult, String> {
        let ssh_config = self.ssh_config();
        crate::storage::ssh::validate_config(&ssh_config)?;
        self.remote_path()?;

        let mut session = crate::storage::ssh::connect(&ssh_config)?;
        crate::storage::ssh::authenticate(&ssh_config, &mut session)?;
        self.ensure_remote_directory(&session)?;
        let remote_path = self.remote_path()?;
        self.upload(&session, &remote_path)?;
        info!(pack_tag = %tag(LogTag::Scp), "Store succeeded: {remote_path}");

        let deleted_file_keys = delete_old_backups(delete_after_upload, |file_key| {
            let remote_path = remote_file_path_from_key(&self.config.path, file_key)?;
            delete_remote_file(&session, &remote_path)
        });

        Ok(StorageRunResult { deleted_file_keys })
    }

    /// Build the shared SSH connection settings used by the SCP backend.
    fn ssh_config(&self) -> SshConnectionConfig<'_> {
        SshConnectionConfig {
            storage_name: "SCP",
            log_tag: LogTag::Scp,
            host: &self.config.host,
            port: self.config.port,
            timeout: self.config.timeout,
            username: &self.config.username,
            password: self.config.password.as_deref(),
            private_key: self.config.private_key.as_deref(),
            passphrase: self.config.passphrase.as_deref(),
        }
    }

    /// Upload the local artifact to the final remote path.
    fn upload(&self, session: &ssh2::Session, remote_path: &str) -> Result<(), String> {
        let mut source_file = File::open(self.source_path).map_err(|error| {
            format!(
                "Failed to open SCP source file {:?}: {error}",
                self.source_path
            )
        })?;
        let source_size = source_file
            .metadata()
            .map_err(|error| {
                format!(
                    "Failed to read SCP source file metadata {:?}: {error}",
                    self.source_path
                )
            })?
            .len();

        info!(pack_tag = %tag(LogTag::Scp), "Uploading backup: {remote_path}");
        let started_at = Instant::now();
        let mut remote_file = session
            .scp_send(Path::new(remote_path), 0o644, source_size, None)
            .map_err(|error| format!("Failed to create SCP remote file {remote_path}: {error}"))?;
        let bytes_uploaded = copy_with_buffer(&mut source_file, &mut remote_file)
            .map_err(|error| format!("Failed to upload SCP file to {remote_path}: {error}"))?;
        remote_file
            .send_eof()
            .map_err(|error| format!("Failed to send SCP EOF to {remote_path}: {error}"))?;
        remote_file
            .wait_eof()
            .map_err(|error| format!("Failed to wait SCP EOF for {remote_path}: {error}"))?;
        remote_file
            .close()
            .map_err(|error| format!("Failed to close SCP remote file {remote_path}: {error}"))?;
        remote_file
            .wait_close()
            .map_err(|error| format!("Failed to wait SCP close for {remote_path}: {error}"))?;

        log_upload_duration(bytes_uploaded, source_size, started_at.elapsed());

        Ok(())
    }

    /// Create the configured remote directory and its parents when missing.
    fn ensure_remote_directory(&self, session: &ssh2::Session) -> Result<(), String> {
        for directory in remote_directories(&self.config.path) {
            let command = format!("mkdir -p {}", shell_quote(&directory));
            run_remote_command(session, &command)?;
        }

        Ok(())
    }

    /// Build the final remote path from configured directory and artifact name.
    fn remote_path(&self) -> Result<String, String> {
        remote_file_path(&self.config.path, self.source_path, "SCP")
    }
}

impl Storage for ScpConfig {
    fn keep(&self) -> u32 {
        self.keep
    }

    fn perform(
        &self,
        source_path: &Path,
        delete_after_upload: &[String],
    ) -> Result<StorageRunResult, String> {
        Scp::new(self, source_path).perform(delete_after_upload)
    }
}

/// Copy bytes with a large buffer to reduce costly libssh2 SCP write calls.
fn copy_with_buffer<R: Read, W: Write>(reader: &mut R, writer: &mut W) -> io::Result<u64> {
    let mut buffer = vec![0; SCP_UPLOAD_BUFFER_SIZE];
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

/// Log uploaded size and throughput using decimal MB, like most SCP clients.
fn log_upload_duration(bytes_uploaded: u64, source_size: u64, duration: Duration) {
    let seconds = duration.as_secs_f64();
    let megabytes_uploaded = bytes_uploaded as f64 / 1_000_000.0;

    if seconds > 0.0 {
        info!(
            pack_tag = %tag(LogTag::Scp),
            "Uploaded {:.2} MB in {:.2}s ({:.2} MB/s)",
            megabytes_uploaded,
            seconds,
            megabytes_uploaded / seconds
        );
    } else {
        info!(
            pack_tag = %tag(LogTag::Scp),
            "Uploaded {:.2} MB",
            megabytes_uploaded
        );
    }

    if source_size != bytes_uploaded {
        info!(
            pack_tag = %tag(LogTag::Scp),
            "Local file size changed during upload: expected {} bytes, uploaded {} bytes",
            source_size,
            bytes_uploaded
        );
    }
}

fn delete_remote_file(session: &ssh2::Session, remote_path: &str) -> Result<(), String> {
    let command = format!("rm {}", shell_quote(remote_path));
    run_remote_command(session, &command)
}

fn run_remote_command(session: &ssh2::Session, command: &str) -> Result<(), String> {
    let mut channel = session
        .channel_session()
        .map_err(|error| format!("Failed to create SCP SSH command session: {error}"))?;

    channel
        .exec(command)
        .map_err(|error| format!("Failed to run SCP remote command `{command}`: {error}"))?;

    let mut stdout = String::new();
    channel.read_to_string(&mut stdout).map_err(|error| {
        format!("Failed to read SCP remote command stdout `{command}`: {error}")
    })?;
    let mut stderr = String::new();
    channel
        .stderr()
        .read_to_string(&mut stderr)
        .map_err(|error| {
            format!("Failed to read SCP remote command stderr `{command}`: {error}")
        })?;

    channel
        .wait_eof()
        .map_err(|error| format!("Failed to wait SCP remote command EOF `{command}`: {error}"))?;
    channel
        .close()
        .map_err(|error| format!("Failed to close SCP remote command `{command}`: {error}"))?;
    channel
        .wait_close()
        .map_err(|error| format!("Failed to wait SCP remote command close `{command}`: {error}"))?;

    let exit_status = channel.exit_status().map_err(|error| {
        format!("Failed to read SCP remote command status `{command}`: {error}")
    })?;
    if exit_status != 0 {
        let stderr = stderr.trim();
        if stderr.is_empty() {
            return Err(format!(
                "SCP remote command `{command}` failed with exit status {exit_status}"
            ));
        }

        return Err(format!(
            "SCP remote command `{command}` failed with exit status {exit_status}: {stderr}"
        ));
    }

    Ok(())
}

/// Quote one string for a POSIX-like remote shell.
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
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

    fn valid_config() -> ScpConfig {
        ScpConfig {
            host: "scp.example.com".to_string(),
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
        config.host = "scp.example.com".to_string();
        config.port = 2222;
        let scp = Scp::new(&config, Path::new("backup.tar.gz"));

        assert_eq!(
            crate::storage::remote_address(scp.ssh_config().host, scp.ssh_config().port),
            "scp.example.com:2222"
        );
    }

    #[test]
    fn ssh_config_maps_scp_fields() {
        let mut config = valid_config();
        config.host = "example.com".to_string();
        config.port = 2222;
        config.timeout = 42;
        config.username = "deploy".to_string();
        config.password = Some("secret".to_string());
        config.private_key = Some("~/.ssh/id_ed25519".to_string());
        config.passphrase = Some("key-passphrase".to_string());
        let scp = Scp::new(&config, Path::new("backup.tar.gz"));

        let ssh_config = scp.ssh_config();

        assert_eq!(ssh_config.storage_name, "SCP");
        assert_eq!(ssh_config.log_tag, LogTag::Scp);
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
        let scp = Scp::new(&config, source_path);

        assert_eq!(
            scp.remote_path().unwrap(),
            "/backups/my_app-20260616-120000.tar.gz"
        );
    }

    #[test]
    fn perform_validates_config_before_connecting() {
        let mut config = valid_config();
        config.host = "".to_string();
        let scp = Scp::new(&config, Path::new("backup.tar.gz"));

        let result = scp.perform(&[]);

        assert!(matches!(result, Err(error) if error.contains("SCP host")));
    }

    #[test]
    fn shell_quote_wraps_plain_path() {
        assert_eq!(shell_quote("/backups/pack"), "'/backups/pack'");
    }

    #[test]
    fn shell_quote_escapes_single_quotes() {
        assert_eq!(
            shell_quote("/backups/app's data"),
            "'/backups/app'\\''s data'"
        );
    }

    #[test]
    fn copy_with_buffer_copies_all_bytes() {
        let input = b"hello scp upload";
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

    #[test]
    fn remote_file_path_from_key_rejects_unsafe_delete_key() {
        assert!(remote_file_path_from_key("/backups", "../backup.tar.gz").is_err());
    }
}
