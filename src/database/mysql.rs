use crate::database::{Database, dump_file_path};
use serde::Deserialize;
use std::path::Path;
use std::process::Command;

/// Configuration specific to MySQL.
#[derive(Debug, Deserialize)]
pub struct MySQLConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_mysql_port")]
    pub port: u16,
    pub database: String,
    #[serde(default = "default_mysql_username")]
    pub username: String,
    pub password: Option<String>,
}

fn default_host() -> String {
    "localhost".to_string()
}

fn default_mysql_port() -> u16 {
    3306
}

fn default_mysql_username() -> String {
    "root".to_string()
}

/// MySQL dump using mysqldump subprocess.
///
/// Builds a command like:
///   mysqldump --host localhost --port 3306 -u root -psecret my_database --result-file=/tmp/dump.sql
pub struct MySQL<'a> {
    config: &'a MySQLConfig,
    dump_path: &'a Path,
}

impl Database for MySQLConfig {
    fn perform(&self, dump_path: &Path) -> Result<(), String> {
        let mysql = MySQL::new(self, dump_path);
        mysql.perform()
    }
}

impl<'a> MySQL<'a> {
    /// Create a new MySQL dump handler.
    ///
    /// * `config` — parsed MySQL config from the YAML file
    /// * `dump_path` — directory where the SQL dump file will be written
    pub fn new(config: &'a MySQLConfig, dump_path: &'a Path) -> Self {
        Self { config, dump_path }
    }

    /// Build the list of arguments for `mysqldump`.
    ///
    /// This is a separate function so we can test the argument construction
    /// without actually running mysqldump.
    pub fn build_args(&self) -> Result<Vec<String>, String> {
        let mut args = vec![
            // Host (default "localhost" from config.rs)
            "--host".to_string(),
            self.config.host.clone(),
            // Port (default 3306 from config.rs)
            "--port".to_string(),
            self.config.port.to_string(),
        ];

        // Authentication
        if !self.config.username.is_empty() {
            args.push("-u".to_string());
            args.push(self.config.username.clone());
        }
        if let Some(ref password) = self.config.password {
            // mysqldump accepts -pPASSWORD without space
            args.push(format!("-p{password}"));
        }

        // Database name — if not set, mysqldump will complain
        args.push(self.config.database.clone());

        // Output file named after the database
        let output_file = dump_file_path(self.dump_path, &self.config.database)?;
        args.push("--result-file".to_string());
        args.push(output_file.to_string_lossy().into_owned());

        Ok(args)
    }

    /// Run `mysqldump` with the configured arguments.
    ///
    /// Returns `Ok(())` on success, or `Err(message)` if the subprocess fails.
    pub fn perform(&self) -> Result<(), String> {
        let args = self.build_args()?;

        let output = Command::new("mysqldump")
            .args(&args)
            .output()
            .map_err(|e| format!("Failed to run mysqldump: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("mysqldump failed:\n{stderr}"));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(overrides: impl FnOnce(&mut MySQLConfig)) -> MySQLConfig {
        let mut config = MySQLConfig {
            host: "localhost".to_string(),
            port: 3306,
            database: "testdb".to_string(),
            username: "root".to_string(),
            password: Some("secret".to_string()),
        };
        overrides(&mut config);
        config
    }

    #[test]
    fn build_args_uses_given_host_and_port() {
        let config = make_config(|c| {
            c.host = "db.example.com".to_string();
            c.port = 3307;
        });
        let mysql = MySQL::new(&config, Path::new("/tmp/dumps"));

        let args = mysql.build_args().unwrap();
        assert!(args.contains(&"--host".to_string()));
        assert!(args.contains(&"db.example.com".to_string()));
        assert!(args.contains(&"--port".to_string()));
        assert!(args.contains(&"3307".to_string()));
    }

    #[test]
    fn build_args_uses_default_port() {
        let config = make_config(|_| {});
        let mysql = MySQL::new(&config, Path::new("/tmp/dumps"));

        let args = mysql.build_args().unwrap();
        assert!(args.contains(&"--port".to_string()));
        assert!(args.contains(&"3306".to_string()));
    }

    #[test]
    fn build_args_includes_auth() {
        let config = make_config(|_| {});
        let mysql = MySQL::new(&config, Path::new("/tmp/dumps"));

        let args = mysql.build_args().unwrap();
        // -u root
        let u_idx = args.iter().position(|a| a == "-u").unwrap();
        assert_eq!(args[u_idx + 1], "root");
        // -psecret
        assert!(args.contains(&"-psecret".to_string()));
    }

    #[test]
    fn build_args_omits_auth_when_not_configured() {
        let config = make_config(|c| {
            c.username = String::new();
            c.password = None;
        });
        let mysql = MySQL::new(&config, Path::new("/tmp/dumps"));

        let args = mysql.build_args().unwrap();
        assert!(!args.contains(&"-u".to_string()));
    }

    #[test]
    fn build_args_uses_database_name() {
        let config = make_config(|c| {
            c.database = "my_app_prod".to_string();
        });
        let mysql = MySQL::new(&config, Path::new("/tmp/dumps"));

        let args = mysql.build_args().unwrap();
        assert!(args.contains(&"my_app_prod".to_string()));
    }

    #[test]
    fn build_args_includes_result_file() {
        let config = make_config(|_| {});
        let mysql = MySQL::new(&config, Path::new("/tmp/dumps"));

        let args = mysql.build_args().unwrap();
        let rf_idx = args.iter().position(|a| a == "--result-file").unwrap();
        assert_eq!(args[rf_idx + 1], "/tmp/dumps/testdb.sql");
    }

    #[test]
    fn build_args_rejects_unsafe_database_name() {
        let config = make_config(|config| {
            config.database = "../escaped".to_string();
        });
        let mysql = MySQL::new(&config, Path::new("/tmp/dumps"));

        assert!(mysql.build_args().is_err());
    }
}
