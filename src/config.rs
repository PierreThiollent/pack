use serde::Deserialize;
use std::collections::HashMap;

/// Entry point of the YAML configuration file
#[derive(Debug, Deserialize)]
pub struct Config {
    /// Backup models, keyed by name (e.g. "my_app")
    pub models: HashMap<String, Model>,
}

/// A model = one complete backup job
#[derive(Debug, Deserialize)]
pub struct Model {
    /// Databases to back up, keyed by name
    #[serde(default)]
    pub databases: HashMap<String, DatabaseConfig>,
}

/// Configuration for a database — the `type` field determines which variant is used
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum DatabaseConfig {
    #[serde(rename = "mysql")]
    MySQL(MySQLConfig),
}

/// Configuration specific to MySQL
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

/// Get the user's home directory.
fn home_dir() -> String {
    std::env::var("HOME").expect("Could not find HOME environment variable")
}

/// Resolve the config file path.
/// If `config_arg` is `Some`, use it (with tilde expansion).
/// Otherwise, default to `$HOME/.rbak/rbak.yml`.
pub fn resolve_config_path(config_arg: Option<String>) -> String {
    let path = config_arg.unwrap_or_else(|| format!("{}/.rbak/rbak.yml", home_dir()));
    // Expand leading tilde (e.g. ~/foo → $HOME/foo)
    match path.strip_prefix("~/") {
        Some(rest) => format!("{}/{rest}", home_dir()),
        None => path,
    }
}

/// Load and parse the config file from the given path.
/// Exits the process with an error message on failure.
pub fn load_config(path: &str) -> Config {
    let yaml_content = std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("Error reading config file {path}: {e}");
        std::process::exit(1);
    });
    serde_yaml::from_str(&yaml_content).unwrap_or_else(|e| {
        eprintln!("Error parsing config file {path}: {e}");
        std::process::exit(1);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mysql_with_all_fields() {
        let yaml = r#"
models:
  my_app:
    databases:
      my_db:
        type: mysql
        host: db.example.com
        port: 3307
        database: my_production_db
        username: root
        password: secret123
"#;

        let config: Config = serde_yaml::from_str(yaml).unwrap();
        let db = config
            .models
            .get("my_app")
            .unwrap()
            .databases
            .get("my_db")
            .unwrap();

        match db {
            DatabaseConfig::MySQL(cfg) => {
                assert_eq!(cfg.host, "db.example.com");
                assert_eq!(cfg.port, 3307);
                assert_eq!(cfg.database, "my_production_db");
                assert_eq!(cfg.username, "root");
                assert_eq!(cfg.password.as_deref(), Some("secret123"));
            }
        }
    }

    #[test]
    fn parse_mysql_with_default_host() {
        let yaml = r#"
models:
  my_app:
    databases:
      my_db:
        type: mysql
        database: ""
"#;

        let config: Config = serde_yaml::from_str(yaml).unwrap();
        let db = config
            .models
            .get("my_app")
            .unwrap()
            .databases
            .get("my_db")
            .unwrap();

        match db {
            DatabaseConfig::MySQL(cfg) => {
                assert_eq!(cfg.host, "localhost");
                assert_eq!(cfg.port, 3306);
                assert_eq!(cfg.database, "");
            }
        }
    }

    #[test]
    fn parse_invalid_yaml_missing_type() {
        let yaml = r#"
models:
  my_app:
    databases:
      my_db:
        host: localhost
"#;

        let result = serde_yaml::from_str::<Config>(yaml);

        assert!(result.is_err());
    }

    #[test]
    fn parse_invalid_yaml_unknown_type() {
        let yaml = r#"
models:
  my_app:
    databases:
      my_db:
        type: postgresql
"#;

        let result = serde_yaml::from_str::<Config>(yaml);

        assert!(result.is_err());
    }

    #[test]
    fn parse_config_without_databases() {
        let yaml = r#"
models:
  my_app:
    databases: {}
"#;

        let config: Config = serde_yaml::from_str(yaml).unwrap();
        let model = config.models.get("my_app").unwrap();
        assert!(model.databases.is_empty());
    }

    #[test]
    fn parse_invalid_yaml_missing_models() {
        let yaml = r#"
foo: bar
"#;

        let result = serde_yaml::from_str::<Config>(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn resolve_default_config_path() {
        let home = std::env::var("HOME").unwrap();
        let path = resolve_config_path(None);
        assert_eq!(path, format!("{home}/.rbak/rbak.yml"));
    }

    #[test]
    fn resolve_config_path_with_tilde() {
        let home = std::env::var("HOME").unwrap();
        let path = resolve_config_path(Some("~/custom/path.yml".to_string()));
        assert_eq!(path, format!("{home}/custom/path.yml"));
    }

    #[test]
    fn resolve_config_path_absolute() {
        let path = resolve_config_path(Some("/etc/rbak.yml".to_string()));
        assert_eq!(path, "/etc/rbak.yml");
    }

    #[test]
    fn resolve_config_path_relative() {
        let path = resolve_config_path(Some("./local.yml".to_string()));
        assert_eq!(path, "./local.yml");
    }
}
