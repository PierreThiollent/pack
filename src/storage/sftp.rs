use serde::Deserialize;
use ssh2::{Session, Sftp as Ssh2Sftp};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::Duration;
use tracing::info;

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
}

#[derive(Debug, PartialEq, Eq)]
enum SftpAuthMethod {
    Password,
    PrivateKey,
}

pub struct Sftp<'a> {
    config: &'a SftpConfig,
    source_path: &'a Path,
}

impl<'a> Sftp<'a> {
    pub fn new(config: &'a SftpConfig, source_path: &'a Path) -> Self {
        Self {
            config,
            source_path,
        }
    }

    pub fn perform(&self) -> Result<(), String> {
        self.validate_config()?;

        let mut session = self.connect_ssh()?;
        self.authenticate(&mut session)?;
        let sftp_session = self.open_sftp_subsystem(&session)?;
        drop(sftp_session);

        Err("SFTP upload is not implemented yet".to_string())
    }

    fn validate_config(&self) -> Result<(), String> {
        if self.config.host.trim().is_empty() || self.config.username.trim().is_empty() {
            return Err("SFTP host or username cannot be empty".to_string());
        }

        if self.config.password.is_none() && self.config.private_key.is_none() {
            return Err("SFTP password or private_key is required".to_string());
        }

        Ok(())
    }

    fn connect_ssh(&self) -> Result<Session, String> {
        let tcp_stream = self.open_tcp_connection()?;
        let mut session = Session::new()
            .map_err(|error| format!("Failed to create SFTP SSH session: {error}"))?;
        session.set_tcp_stream(tcp_stream);
        session
            .handshake()
            .map_err(|error| format!("Failed to start SFTP SSH session: {error}"))?;

        info!("[SFTP] SSH session established");
        Ok(session)
    }

    fn authenticate(&self, session: &mut Session) -> Result<(), String> {
        match self.auth_method()? {
            SftpAuthMethod::Password => self.authenticate_with_password(session),
            SftpAuthMethod::PrivateKey => {
                Err("SFTP private_key authentication is not implemented yet".to_string())
            }
        }
    }

    fn auth_method(&self) -> Result<SftpAuthMethod, String> {
        if self.config.password.is_some() {
            return Ok(SftpAuthMethod::Password);
        }

        if self.config.private_key.is_some() {
            return Ok(SftpAuthMethod::PrivateKey);
        }

        Err("SFTP password or private_key is required".to_string())
    }

    fn authenticate_with_password(&self, session: &mut Session) -> Result<(), String> {
        let password =
            self.config.password.as_ref().ok_or_else(|| {
                "SFTP password is required for password authentication".to_string()
            })?;

        session
            .userauth_password(&self.config.username, password)
            .map_err(|error| {
                format!("Failed to authenticate to SFTP server with password: {error}")
            })?;

        if session.authenticated() {
            info!("[SFTP] Authenticated with password");
            return Ok(());
        }

        Err("Failed to authenticate to SFTP server with password".to_string())
    }

    fn open_sftp_subsystem(&self, session: &Session) -> Result<Ssh2Sftp, String> {
        let sftp_session = session
            .sftp()
            .map_err(|error| format!("Failed to open SFTP subsystem: {error}"))?;

        info!("[SFTP] SFTP session opened");
        Ok(sftp_session)
    }

    fn open_tcp_connection(&self) -> Result<TcpStream, String> {
        info!("[SFTP] Connecting to {}", self.remote_address());

        TcpStream::connect_timeout(&self.socket_address()?, self.timeout())
            .map_err(|error| format!("Failed to connect to SFTP server: {error}"))
    }

    fn socket_address(&self) -> Result<std::net::SocketAddr, String> {
        self.remote_address()
            .to_socket_addrs()
            .map_err(|error| format!("Failed to resolve SFTP server address: {error}"))?
            .next()
            .ok_or_else(|| "Failed to resolve SFTP server address".to_string())
    }

    fn remote_address(&self) -> String {
        format!("{}:{}", self.config.host, self.config.port)
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(self.config.timeout)
    }

    fn remote_path(&self) -> Result<String, String> {
        crate::storage::remote_file_path(&self.config.path, self.source_path, "SFTP")
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
        }
    }

    #[test]
    fn remote_address_uses_host_and_port() {
        let mut config = valid_config();
        config.host = "sftp.example.com".to_string();
        config.port = 2222;
        let source_path = Path::new("backup.tar.gz");
        let sftp = Sftp::new(&config, source_path);

        assert_eq!(sftp.remote_address(), "sftp.example.com:2222");
    }

    #[test]
    fn timeout_uses_configured_seconds() {
        let mut config = valid_config();
        config.timeout = 42;
        let source_path = Path::new("backup.tar.gz");
        let sftp = Sftp::new(&config, source_path);

        assert_eq!(sftp.timeout(), Duration::from_secs(42));
    }

    #[test]
    fn auth_method_uses_password_when_password_is_configured() {
        let config = valid_config();
        let source_path = Path::new("backup.tar.gz");
        let sftp = Sftp::new(&config, source_path);

        assert_eq!(sftp.auth_method().unwrap(), SftpAuthMethod::Password);
    }

    #[test]
    fn auth_method_uses_private_key_when_password_is_missing() {
        let mut config = valid_config();
        config.password = None;
        config.private_key = Some("~/.ssh/id_rsa".to_string());
        let source_path = Path::new("backup.tar.gz");
        let sftp = Sftp::new(&config, source_path);

        assert_eq!(sftp.auth_method().unwrap(), SftpAuthMethod::PrivateKey);
    }

    #[test]
    fn auth_method_prefers_password_when_both_are_configured() {
        let mut config = valid_config();
        config.private_key = Some("~/.ssh/id_rsa".to_string());
        let source_path = Path::new("backup.tar.gz");
        let sftp = Sftp::new(&config, source_path);

        assert_eq!(sftp.auth_method().unwrap(), SftpAuthMethod::Password);
    }

    #[test]
    fn validate_config_accepts_password_auth() {
        let config = valid_config();
        let source_path = Path::new("backup.tar.gz");
        let sftp = Sftp::new(&config, source_path);

        assert!(sftp.validate_config().is_ok());
    }

    #[test]
    fn validate_config_accepts_private_key_auth() {
        let mut config = valid_config();
        config.password = None;
        config.private_key = Some("~/.ssh/id_rsa".to_string());
        let source_path = Path::new("backup.tar.gz");
        let sftp = Sftp::new(&config, source_path);

        assert!(sftp.validate_config().is_ok());
    }

    #[test]
    fn validate_config_rejects_empty_required_fields() {
        let mut config = valid_config();
        config.host = "".to_string();
        let source_path = Path::new("backup.tar.gz");
        let sftp = Sftp::new(&config, source_path);

        assert!(sftp.validate_config().is_err());
    }

    #[test]
    fn validate_config_rejects_missing_auth_method() {
        let mut config = valid_config();
        config.password = None;
        config.private_key = None;
        let source_path = Path::new("backup.tar.gz");
        let sftp = Sftp::new(&config, source_path);

        assert!(sftp.validate_config().is_err());
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
}
