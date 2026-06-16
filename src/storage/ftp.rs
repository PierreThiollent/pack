use crate::storage::{remote_directories, remote_file_path};
use serde::Deserialize;
use std::fs::File;
use std::net::ToSocketAddrs;
use std::path::Path;
use std::time::Duration;
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
}

pub struct Ftp<'a> {
    config: &'a FtpConfig,
    source_path: &'a Path,
}

impl<'a> Ftp<'a> {
    pub fn new(config: &'a FtpConfig, source_path: &'a Path) -> Self {
        Self {
            config,
            source_path,
        }
    }

    pub fn perform(&self) -> Result<(), String> {
        self.validate_config()?;

        if self.config.explicit_tls {
            let mut ftp_stream = self.open_explicit_tls()?;
            self.perform_with_stream(&mut ftp_stream)
        } else {
            let mut ftp_stream = self.open_plain()?;
            self.perform_with_stream(&mut ftp_stream)
        }
    }

    fn perform_with_stream<T: TlsStream>(
        &self,
        ftp_stream: &mut ImplFtpStream<T>,
    ) -> Result<(), String> {
        self.ensure_remote_directory(ftp_stream)?;
        let remote_path = self.remote_path()?;
        self.upload(ftp_stream, &remote_path)?;
        ftp_stream
            .quit()
            .map_err(|error| format!("Failed to close FTP connection: {error}"))?;

        info!("[FTP] Store succeeded: {remote_path}");
        Ok(())
    }

    fn open_plain(&self) -> Result<FtpStream, String> {
        info!(
            "[FTP] Connecting to {} with remote path {}",
            self.remote_address(),
            self.config.path
        );

        let mut ftp_stream = FtpStream::connect_timeout(self.socket_address()?, self.timeout())
            .map_err(|error| format!("Failed to connect to FTP server: {error}"))?;

        ftp_stream
            .login(&self.config.username, &self.config.password)
            .map_err(|error| format!("Failed to login to FTP server: {error}"))?;

        Ok(ftp_stream)
    }

    fn open_explicit_tls(&self) -> Result<NativeTlsFtpStream, String> {
        info!(
            "[FTP] Connecting to {} with explicit TLS and remote path {}",
            self.remote_address(),
            self.config.path
        );

        let ftp_stream =
            NativeTlsFtpStream::connect_timeout(self.socket_address()?, self.timeout())
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

    fn socket_address(&self) -> Result<std::net::SocketAddr, String> {
        self.remote_address()
            .to_socket_addrs()
            .map_err(|error| format!("Failed to resolve FTP server address: {error}"))?
            .next()
            .ok_or_else(|| "Failed to resolve FTP server address".to_string())
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(self.config.timeout)
    }

    fn ensure_remote_directory<T: TlsStream>(
        &self,
        ftp_stream: &mut ImplFtpStream<T>,
    ) -> Result<(), String> {
        for directory in remote_directories(&self.config.path) {
            if ftp_stream.cwd(&directory).is_ok() {
                continue;
            }

            info!("[FTP] Creating remote directory: {directory}");
            ftp_stream
                .mkdir(&directory)
                .map_err(|error| format!("Failed to create FTP directory {directory}: {error}"))?;
        }

        Ok(())
    }

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

        info!("[FTP] Uploading backup: {remote_path}");
        ftp_stream
            .put_file(remote_path, &mut source_file)
            .map_err(|error| format!("Failed to upload FTP file to {remote_path}: {error}"))?;

        Ok(())
    }

    fn remote_address(&self) -> String {
        format!("{}:{}", self.config.host, self.config.port)
    }

    fn remote_path(&self) -> Result<String, String> {
        remote_file_path(&self.config.path, self.source_path, "FTP")
    }

    fn validate_config(&self) -> Result<(), String> {
        if self.config.host.trim().is_empty()
            || self.config.username.trim().is_empty()
            || self.config.password.trim().is_empty()
        {
            return Err("FTP host, username or password cannot be empty".to_string());
        }

        Ok(())
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
        }
    }

    #[test]
    fn validate_config_accepts_required_fields() {
        let config = valid_config();
        let source_path = Path::new("backup.tar.gz");
        let ftp = Ftp::new(&config, source_path);

        assert!(ftp.validate_config().is_ok());
    }

    #[test]
    fn validate_config_rejects_empty_required_fields() {
        let mut config = valid_config();
        config.host = "".to_string();
        let source_path = Path::new("backup.tar.gz");
        let ftp = Ftp::new(&config, source_path);

        assert!(ftp.validate_config().is_err());
    }

    #[test]
    fn remote_address_uses_host_and_port() {
        let mut config = valid_config();
        config.host = "ftp.example.com".to_string();
        config.port = 2121;
        let source_path = Path::new("backup.tar.gz");
        let ftp = Ftp::new(&config, source_path);

        assert_eq!(ftp.remote_address(), "ftp.example.com:2121");
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
