use crate::storage::remote_directories;
use serde::Deserialize;
use std::fs::File;
use std::net::ToSocketAddrs;
use std::path::Path;
use std::time::Duration;
use suppaftp::FtpStream;
use suppaftp::types::FileType;
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
    pub tls: bool,

    #[serde(default)]
    pub explicit_tls: bool,

    #[serde(default)]
    pub no_check_certificate: bool,

    #[serde(default)]
    pub keep: u32,
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

        if self.config.tls || self.config.explicit_tls {
            return Err(format!(
                "FTP TLS is not implemented yet (no_check_certificate={})",
                self.config.no_check_certificate
            ));
        }

        info!(
            "[FTP] Connecting to {} with remote path {} (keep={})",
            self.remote_address(),
            self.config.path,
            self.config.keep
        );

        let mut ftp_stream = self.open()?;
        self.ensure_remote_directory(&mut ftp_stream)?;
        let remote_path = self.remote_path()?;
        self.upload(&mut ftp_stream, &remote_path)?;
        ftp_stream
            .quit()
            .map_err(|error| format!("Failed to close FTP connection: {error}"))?;

        info!("[FTP] Store succeeded: {remote_path}");
        Ok(())
    }

    fn open(&self) -> Result<FtpStream, String> {
        let timeout = Duration::from_secs(self.config.timeout);
        let socket_address = self
            .remote_address()
            .to_socket_addrs()
            .map_err(|error| format!("Failed to resolve FTP server address: {error}"))?
            .next()
            .ok_or_else(|| "Failed to resolve FTP server address".to_string())?;

        let mut ftp_stream = FtpStream::connect_timeout(socket_address, timeout)
            .map_err(|error| format!("Failed to connect to FTP server: {error}"))?;

        ftp_stream
            .login(&self.config.username, &self.config.password)
            .map_err(|error| format!("Failed to login to FTP server: {error}"))?;

        Ok(ftp_stream)
    }

    fn ensure_remote_directory(&self, ftp_stream: &mut FtpStream) -> Result<(), String> {
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

    fn upload(&self, ftp_stream: &mut FtpStream, remote_path: &str) -> Result<(), String> {
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
        let file_name = self
            .source_path
            .file_name()
            .and_then(|file_name| file_name.to_str())
            .ok_or_else(|| format!("FTP source path has no file name: {:?}", self.source_path))?;

        let remote_directory = self.config.path.trim_end_matches('/');
        if remote_directory.is_empty() {
            Ok(format!("/{file_name}"))
        } else {
            Ok(format!("{remote_directory}/{file_name}"))
        }
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
            tls: false,
            explicit_tls: false,
            no_check_certificate: false,
            keep: 0,
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
    fn remote_path_uses_configured_directory() {
        let config = valid_config();
        let source_path = Path::new("/tmp/my_app-20260616-120000.tar.gz");
        let ftp = Ftp::new(&config, source_path);

        assert_eq!(
            ftp.remote_path().unwrap(),
            "/backups/my_app-20260616-120000.tar.gz"
        );
    }

    #[test]
    fn remote_path_uses_root_directory() {
        let mut config = valid_config();
        config.path = "/".to_string();
        let source_path = Path::new("/tmp/my_app-20260616-120000.tar.gz");
        let ftp = Ftp::new(&config, source_path);

        assert_eq!(ftp.remote_path().unwrap(), "/my_app-20260616-120000.tar.gz");
    }

    #[test]
    fn remote_path_trims_trailing_slash() {
        let mut config = valid_config();
        config.path = "/backups/".to_string();
        let source_path = Path::new("/tmp/my_app-20260616-120000.tar.gz");
        let ftp = Ftp::new(&config, source_path);

        assert_eq!(
            ftp.remote_path().unwrap(),
            "/backups/my_app-20260616-120000.tar.gz"
        );
    }

    #[test]
    fn remote_path_rejects_source_without_file_name() {
        let config = valid_config();
        let source_path = Path::new("/");
        let ftp = Ftp::new(&config, source_path);

        assert!(ftp.remote_path().is_err());
    }

    #[test]
    fn perform_rejects_tls_for_now() {
        let mut config = valid_config();
        config.tls = true;
        let source_path = Path::new("backup.tar.gz");
        let ftp = Ftp::new(&config, source_path);

        let result = ftp.perform();

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("TLS"));
    }
}
