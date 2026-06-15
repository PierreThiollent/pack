use serde::Deserialize;

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

        let _configuration_summary = format!(
            "{}:{}{} timeout={} tls={} explicit_tls={} no_check_certificate={} keep={}",
            self.config.host,
            self.config.port,
            self.config.path,
            self.config.timeout,
            self.config.tls,
            self.config.explicit_tls,
            self.config.no_check_certificate,
            self.config.keep
        );

        Err("FTP storage is not implemented yet".to_string())
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
}
