use serde::Deserialize;
use std::net::ToSocketAddrs;
use std::time::Duration;
use suppaftp::FtpStream;
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
}

impl<'a> Ftp<'a> {
    pub fn new(config: &'a FtpConfig) -> Self {
        Self { config }
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
        ftp_stream
            .quit()
            .map_err(|error| format!("Failed to close FTP connection: {error}"))?;

        info!("[FTP] Connection succeeded: {}", self.remote_address());
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

    fn remote_address(&self) -> String {
        format!("{}:{}", self.config.host, self.config.port)
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
        let ftp = Ftp::new(&config);

        assert!(ftp.validate_config().is_ok());
    }

    #[test]
    fn validate_config_rejects_empty_required_fields() {
        let mut config = valid_config();
        config.host = "".to_string();
        let ftp = Ftp::new(&config);

        assert!(ftp.validate_config().is_err());
    }

    #[test]
    fn remote_address_uses_host_and_port() {
        let mut config = valid_config();
        config.host = "ftp.example.com".to_string();
        config.port = 2121;
        let ftp = Ftp::new(&config);

        assert_eq!(ftp.remote_address(), "ftp.example.com:2121");
    }

    #[test]
    fn perform_rejects_tls_for_now() {
        let mut config = valid_config();
        config.tls = true;
        let ftp = Ftp::new(&config);

        let result = ftp.perform();

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("TLS"));
    }
}
