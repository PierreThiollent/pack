use crate::database::{Database, dump_file_path};
use serde::Deserialize;
use std::path::Path;
use std::process::Command;

/// Configuration specific to PostgreSQL.
#[derive(Debug, Deserialize)]
pub struct PostgreSQLConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_postgresql_port")]
    pub port: u16,
    pub database: String,
    #[serde(default = "default_postgresql_username")]
    pub username: String,
    pub password: Option<String>,
}

fn default_host() -> String {
    "localhost".to_string()
}

fn default_postgresql_port() -> u16 {
    5432
}

fn default_postgresql_username() -> String {
    "postgres".to_string()
}

impl Database for PostgreSQLConfig {
    fn perform(&self, dump_path: &Path) -> Result<(), String> {
        let postgresql = PostgreSQL::new(self, dump_path);
        postgresql.perform()
    }
}

/// PostgreSQL dump using pg_dump subprocess.
///
/// Builds a command like:
///   pg_dump --host localhost --port 5432 --username postgres --file /tmp/dump.sql my_database
pub struct PostgreSQL<'a> {
    config: &'a PostgreSQLConfig,
    dump_path: &'a Path,
}

impl<'a> PostgreSQL<'a> {
    /// Create a new PostgreSQL dump handler.
    ///
    /// * `config` — parsed PostgreSQL config from the YAML file
    /// * `dump_path` — directory where the SQL dump file will be written
    pub fn new(config: &'a PostgreSQLConfig, dump_path: &'a Path) -> Self {
        Self { config, dump_path }
    }

    /// Build the list of arguments for `pg_dump`.
    ///
    /// This is a separate function so we can test the argument construction
    /// without actually running pg_dump.
    pub fn build_args(&self) -> Result<Vec<String>, String> {
        let mut args = vec![
            "--host".to_string(),
            self.config.host.clone(),
            "--port".to_string(),
            self.config.port.to_string(),
        ];

        if !self.config.username.is_empty() {
            args.push("--username".to_string());
            args.push(self.config.username.clone());
        }

        let output_file = dump_file_path(self.dump_path, &self.config.database)?;
        args.push("--file".to_string());
        args.push(output_file.to_string_lossy().into_owned());

        args.push(self.config.database.clone());

        Ok(args)
    }

    /// Run `pg_dump` with the configured arguments.
    ///
    /// Returns `Ok(())` on success, or `Err(message)` if the subprocess fails.
    pub fn perform(&self) -> Result<(), String> {
        let args = self.build_args()?;
        let mut command = Command::new("pg_dump");
        command.args(&args);

        if let Some(password) = &self.config.password {
            command.env("PGPASSWORD", password);
        }

        let output = command
            .output()
            .map_err(|error| format!("Failed to run pg_dump: {error}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("pg_dump failed:\n{stderr}"));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(overrides: impl FnOnce(&mut PostgreSQLConfig)) -> PostgreSQLConfig {
        let mut config = PostgreSQLConfig {
            host: "localhost".to_string(),
            port: 5432,
            database: "testdb".to_string(),
            username: "postgres".to_string(),
            password: Some("secret".to_string()),
        };
        overrides(&mut config);
        config
    }

    #[test]
    fn build_args_uses_given_host_and_port() {
        let config = make_config(|config| {
            config.host = "db.example.com".to_string();
            config.port = 5433;
        });
        let postgresql = PostgreSQL::new(&config, Path::new("/tmp/dumps"));

        let args = postgresql.build_args().unwrap();

        assert!(args.contains(&"--host".to_string()));
        assert!(args.contains(&"db.example.com".to_string()));
        assert!(args.contains(&"--port".to_string()));
        assert!(args.contains(&"5433".to_string()));
    }

    #[test]
    fn build_args_uses_default_port() {
        let config = make_config(|_| {});
        let postgresql = PostgreSQL::new(&config, Path::new("/tmp/dumps"));

        let args = postgresql.build_args().unwrap();

        assert!(args.contains(&"--port".to_string()));
        assert!(args.contains(&"5432".to_string()));
    }

    #[test]
    fn build_args_includes_username() {
        let config = make_config(|_| {});
        let postgresql = PostgreSQL::new(&config, Path::new("/tmp/dumps"));

        let args = postgresql.build_args().unwrap();
        let username_index = args.iter().position(|arg| arg == "--username").unwrap();

        assert_eq!(args[username_index + 1], "postgres");
    }

    #[test]
    fn build_args_omits_username_when_not_configured() {
        let config = make_config(|config| {
            config.username = String::new();
        });
        let postgresql = PostgreSQL::new(&config, Path::new("/tmp/dumps"));

        let args = postgresql.build_args().unwrap();

        assert!(!args.contains(&"--username".to_string()));
    }

    #[test]
    fn build_args_does_not_include_password() {
        let config = make_config(|_| {});
        let postgresql = PostgreSQL::new(&config, Path::new("/tmp/dumps"));

        let args = postgresql.build_args().unwrap();

        assert!(!args.contains(&"secret".to_string()));
        assert!(!args.iter().any(|arg| arg.contains("PGPASSWORD")));
    }

    #[test]
    fn build_args_uses_database_name() {
        let config = make_config(|config| {
            config.database = "my_app_prod".to_string();
        });
        let postgresql = PostgreSQL::new(&config, Path::new("/tmp/dumps"));

        let args = postgresql.build_args().unwrap();

        assert!(args.contains(&"my_app_prod".to_string()));
    }

    #[test]
    fn build_args_includes_output_file() {
        let config = make_config(|_| {});
        let postgresql = PostgreSQL::new(&config, Path::new("/tmp/dumps"));

        let args = postgresql.build_args().unwrap();
        let file_index = args.iter().position(|arg| arg == "--file").unwrap();

        assert_eq!(args[file_index + 1], "/tmp/dumps/testdb.sql");
    }

    #[test]
    fn build_args_rejects_unsafe_database_name() {
        let config = make_config(|config| {
            config.database = "../escaped".to_string();
        });
        let postgresql = PostgreSQL::new(&config, Path::new("/tmp/dumps"));

        assert!(postgresql.build_args().is_err());
    }
}
