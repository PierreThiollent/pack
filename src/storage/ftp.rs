use crate::logging::{LogTag, tag};
use crate::storage::{
    StorageRunResult, delete_old_backups, remote_address, remote_directories, remote_file_path,
    remote_file_path_from_key, socket_address, timeout_duration,
};
use serde::Deserialize;
use std::fs::File;
use std::path::Path;
use suppaftp::native_tls::TlsConnector;
use suppaftp::types::FileType;
use suppaftp::{FtpStream, ImplFtpStream, NativeTlsConnector, NativeTlsFtpStream, TlsStream};
use tracing::info;

/// Configuration specific to FTP storage.
#[derive(Debug, Deserialize)]
pub struct FtpConfig {
    pub host: String,

    #[serde(default = "default_port")]
    pub port: u16,

    #[serde(default = "default_timeout")]
    pub timeout: u64,

    #[serde(default = "default_path")]
    pub path: String,

    pub username: String,
    pub password: String,

    #[serde(default)]
    pub explicit_tls: bool,

    #[serde(default)]
    pub no_check_certificate: bool,

    #[serde(default)]
    pub keep: u32,
}

/// FTP storage handler for one artifact upload.
pub struct Ftp<'a> {
    config: &'a FtpConfig,
    source_path: &'a Path,
}

impl<'a> Ftp<'a> {
    /// Create a new FTP storage handler.
    pub fn new(config: &'a FtpConfig, source_path: &'a Path) -> Self {
        Self {
            config,
            source_path,
        }
    }

    /// Connect, authenticate, create remote directories and upload the artifact.
    pub fn perform(&self, delete_after_upload: &[String]) -> Result<StorageRunResult, String> {
        self.validate_config()?;

        if self.config.explicit_tls {
            info!(
                pack_tag = %tag(LogTag::Ftp),
                "Connecting to {} with explicit TLS and remote path {}",
                remote_address(&self.config.host, self.config.port),
                self.config.path
            );
            let mut ftp_stream = self.connect_explicit_tls()?;
            self.perform_with_stream(&mut ftp_stream, delete_after_upload)
        } else {
            info!(
                pack_tag = %tag(LogTag::Ftp),
                "Connecting to {} with remote path {}",
                remote_address(&self.config.host, self.config.port),
                self.config.path
            );
            let mut ftp_stream = self.connect_plain()?;
            self.perform_with_stream(&mut ftp_stream, delete_after_upload)
        }
    }

    /// Run the common post-connection flow for plain FTP and explicit TLS FTP.
    fn perform_with_stream<T: TlsStream>(
        &self,
        ftp_stream: &mut ImplFtpStream<T>,
        delete_after_upload: &[String],
    ) -> Result<StorageRunResult, String> {
        self.ensure_remote_directory(ftp_stream)?;
        let remote_path = self.remote_path()?;
        self.upload(ftp_stream, &remote_path)?;
        info!(pack_tag = %tag(LogTag::Ftp), "Store succeeded: {remote_path}");

        let deleted_file_keys = delete_old_backups(delete_after_upload, |file_key| {
            let remote_path = remote_file_path_from_key(&self.config.path, file_key)?;
            delete_remote_file_with_stream(ftp_stream, &remote_path)
        });

        ftp_stream
            .quit()
            .map_err(|error| format!("Failed to close FTP connection: {error}"))?;

        Ok(StorageRunResult { deleted_file_keys })
    }

    /// Create the configured remote directory and its parents when missing.
    fn ensure_remote_directory<T: TlsStream>(
        &self,
        ftp_stream: &mut ImplFtpStream<T>,
    ) -> Result<(), String> {
        for directory in remote_directories(&self.config.path) {
            if ftp_stream.cwd(&directory).is_ok() {
                continue;
            }

            info!(
                pack_tag = %tag(LogTag::Ftp),
                "Creating remote directory: {directory}"
            );
            ftp_stream
                .mkdir(&directory)
                .map_err(|error| format!("Failed to create FTP directory {directory}: {error}"))?;
        }

        Ok(())
    }

    /// Upload the local artifact to the final remote path in binary mode.
    fn upload<T: TlsStream>(
        &self,
        ftp_stream: &mut ImplFtpStream<T>,
        remote_path: &str,
    ) -> Result<(), String> {
        let mut source_file = File::open(self.source_path).map_err(|error| {
            format!(
                "Failed to open FTP source file {:?}: {error}",
                self.source_path
            )
        })?;

        ftp_stream
            .transfer_type(FileType::Binary)
            .map_err(|error| format!("Failed to set FTP binary transfer mode: {error}"))?;

        info!(pack_tag = %tag(LogTag::Ftp), "Uploading backup: {remote_path}");
        ftp_stream
            .put_file(remote_path, &mut source_file)
            .map_err(|error| format!("Failed to upload FTP file to {remote_path}: {error}"))?;

        Ok(())
    }

    /// Validate FTP settings before opening any network connection.
    fn validate_config(&self) -> Result<(), String> {
        if self.config.host.trim().is_empty()
            || self.config.username.trim().is_empty()
            || self.config.password.trim().is_empty()
        {
            return Err("FTP host, username or password cannot be empty".to_string());
        }

        Ok(())
    }

    /// Connect and authenticate with plain FTP.
    fn connect_plain(&self) -> Result<FtpStream, String> {
        let mut ftp_stream = FtpStream::connect_timeout(
            socket_address(&self.config.host, self.config.port, "FTP")?,
            timeout_duration(self.config.timeout),
        )
        .map_err(|error| format!("Failed to connect to FTP server: {error}"))?;

        ftp_stream
            .login(&self.config.username, &self.config.password)
            .map_err(|error| format!("Failed to login to FTP server: {error}"))?;

        Ok(ftp_stream)
    }

    /// Connect, enable explicit TLS, and authenticate.
    fn connect_explicit_tls(&self) -> Result<NativeTlsFtpStream, String> {
        let ftp_stream = NativeTlsFtpStream::connect_timeout(
            socket_address(&self.config.host, self.config.port, "FTP")?,
            timeout_duration(self.config.timeout),
        )
        .map_err(|error| format!("Failed to connect to FTP server: {error}"))?;

        let tls_connector = TlsConnector::builder()
            .danger_accept_invalid_certs(self.config.no_check_certificate)
            .build()
            .map_err(|error| format!("Failed to build FTP TLS connector: {error}"))?;

        let mut ftp_stream = ftp_stream
            .into_secure(NativeTlsConnector::from(tls_connector), &self.config.host)
            .map_err(|error| {
                format!(
                    "FTP server rejected explicit TLS. Disable `explicit_tls` if this server does not support FTPS explicit mode: {error}"
                )
            })?;

        ftp_stream
            .login(&self.config.username, &self.config.password)
            .map_err(|error| format!("Failed to login to FTP server: {error}"))?;

        Ok(ftp_stream)
    }

    /// Build the final remote path from configured directory and artifact name.
    fn remote_path(&self) -> Result<String, String> {
        remote_file_path(&self.config.path, self.source_path, "FTP")
    }
}

fn default_port() -> u16 {
    21
}

fn default_timeout() -> u64 {
    300
}

fn default_path() -> String {
    "/".to_string()
}

fn delete_remote_file_with_stream<T: TlsStream>(
    ftp_stream: &mut ImplFtpStream<T>,
    remote_path: &str,
) -> Result<(), String> {
    ftp_stream
        .rm(remote_path)
        .map_err(|error| format!("Failed to delete FTP file {remote_path}: {error}"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_config() -> FtpConfig {
        FtpConfig {
            host: "ftp.example.com".to_string(),
            port: 21,
            timeout: 300,
            path: "/backups".to_string(),
            username: "user".to_string(),
            password: "secret".to_string(),
            explicit_tls: false,
            no_check_certificate: false,
            keep: 0,
        }
    }

    #[test]
    fn validate_config_accepts_required_fields() {
        let config = valid_config();
        let ftp = Ftp::new(&config, Path::new("backup.tar.gz"));

        assert!(ftp.validate_config().is_ok());
    }

    #[test]
    fn validate_config_rejects_empty_required_fields() {
        let mut config = valid_config();
        config.host = "".to_string();
        let ftp = Ftp::new(&config, Path::new("backup.tar.gz"));

        assert!(ftp.validate_config().is_err());
    }

    #[test]
    fn remote_address_uses_host_and_port() {
        let mut config = valid_config();
        config.host = "ftp.example.com".to_string();
        config.port = 2121;
        let source_path = Path::new("backup.tar.gz");
        let ftp = Ftp::new(&config, source_path);

        assert_eq!(
            remote_address(&ftp.config.host, ftp.config.port),
            "ftp.example.com:2121"
        );
    }

    #[test]
    fn remote_path_uses_shared_remote_file_path_helper() {
        let config = valid_config();
        let source_path = Path::new("/tmp/my_app-20260616-120000.tar.gz");
        let ftp = Ftp::new(&config, source_path);

        assert_eq!(
            ftp.remote_path().unwrap(),
            "/backups/my_app-20260616-120000.tar.gz"
        );
    }
}
