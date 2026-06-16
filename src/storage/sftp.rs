use serde::Deserialize;
use std::path::Path;

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
