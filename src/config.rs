use crate::archive::ArchiveConfig;
use crate::database::mysql::MySQLConfig;
use crate::paths;
use crate::storage::local::LocalConfig;
use serde::Deserialize;
use std::collections::HashMap;
use tracing::error;

/// Entry point of the YAML configuration file
#[derive(Debug, Deserialize)]
pub struct Config {
    /// Directory used to generate temporary backup files.
    pub workdir: Option<String>,

    /// Backup models, keyed by name (e.g. "my_app")
    pub models: HashMap<String, Model>,
}

/// A model = one complete backup job
#[derive(Debug, Deserialize)]
pub struct Model {
    /// Databases to back up, keyed by name
    #[serde(default)]
    pub databases: HashMap<String, DatabaseConfig>,

    /// Storages where backups will be copied or uploaded, keyed by name
    #[serde(default)]
    pub storages: HashMap<String, StorageConfig>,

    /// Files and directories to include in the backup archive
    pub archive: Option<ArchiveConfig>,
}

/// Configuration for a database — the `type` field determines which variant is used
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum DatabaseConfig {
    #[serde(rename = "mysql")]
    MySQL(MySQLConfig),
}

impl DatabaseConfig {
    pub fn type_name(&self) -> &'static str {
        match self {
            DatabaseConfig::MySQL(_) => "MySQL",
        }
    }
}

/// Configuration for a storage — the `type` field determines which variant is used
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum StorageConfig {
    #[serde(rename = "local")]
    Local(LocalConfig),
}

impl StorageConfig {
    pub fn type_name(&self) -> &'static str {
        match self {
            StorageConfig::Local(_) => "Local",
        }
    }
}

/// Resolve the config file path.
/// If `config_arg` is `Some`, use it (with tilde expansion).
/// Otherwise, default to `$HOME/.rbak/rbak.yml`.
pub fn resolve_config_path(config_arg: Option<String>) -> String {
    let path = config_arg.unwrap_or_else(|| format!("{}/.rbak/rbak.yml", paths::home_dir()));
    paths::expand_tilde(&path)
}

/// Load and parse the config file from the given path.
/// Exits the process with an error message on failure.
pub fn load_config(path: &str) -> Config {
    let yaml_content = std::fs::read_to_string(path).unwrap_or_else(|error| {
        error!("Failed to read config file {path}: {error}");
        std::process::exit(1);
    });
    serde_yaml::from_str(&yaml_content).unwrap_or_else(|error| {
        error!("Failed to parse config file {path}: {error}");
        std::process::exit(1);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_config_with_workdir() {
        let yaml = r#"
workdir: /var/tmp/rbak
models: {}
"#;

        let config: Config = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(config.workdir.as_deref(), Some("/var/tmp/rbak"));
    }

    #[test]
    fn parse_config_without_workdir() {
        let yaml = r#"
models: {}
"#;

        let config: Config = serde_yaml::from_str(yaml).unwrap();

        assert!(config.workdir.is_none());
    }

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
    fn database_config_type_name_returns_mysql() {
        let yaml = r#"
models:
  my_app:
    databases:
      my_db:
        type: mysql
        database: app
"#;

        let config: Config = serde_yaml::from_str(yaml).unwrap();
        let database = config
            .models
            .get("my_app")
            .unwrap()
            .databases
            .get("my_db")
            .unwrap();

        assert_eq!(database.type_name(), "MySQL");
    }

    #[test]
    fn storage_config_type_name_returns_local() {
        let yaml = r#"
models:
  my_app:
    storages:
      desktop:
        type: local
        path: /tmp/backups
"#;

        let config: Config = serde_yaml::from_str(yaml).unwrap();
        let storage = config
            .models
            .get("my_app")
            .unwrap()
            .storages
            .get("desktop")
            .unwrap();

        assert_eq!(storage.type_name(), "Local");
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
    fn parse_local_storage() {
        let yaml = r#"
models:
  my_app:
    storages:
      local_backup:
        type: local
        path: ~/Desktop/rbak-backups
"#;

        let config: Config = serde_yaml::from_str(yaml).unwrap();
        let storage = config
            .models
            .get("my_app")
            .unwrap()
            .storages
            .get("local_backup")
            .unwrap();

        match storage {
            StorageConfig::Local(local_config) => {
                assert_eq!(local_config.path, "~/Desktop/rbak-backups");
            }
        }
    }

    #[test]
    fn parse_config_without_storages() {
        let yaml = r#"
models:
  my_app:
    databases: {}
"#;

        let config: Config = serde_yaml::from_str(yaml).unwrap();
        let model = config.models.get("my_app").unwrap();
        assert!(model.storages.is_empty());
    }

    #[test]
    fn parse_invalid_yaml_unknown_storage_type() {
        let yaml = r#"
models:
  my_app:
    storages:
      remote:
        type: s3
        path: somewhere
"#;

        let result = serde_yaml::from_str::<Config>(yaml);

        assert!(result.is_err());
    }

    #[test]
    fn parse_archive_with_includes_and_excludes() {
        let yaml = r#"
models:
  my_app:
    archive:
      includes:
        - ~/Desktop/test
      excludes:
        - ~/Desktop/test/cache
"#;

        let config: Config = serde_yaml::from_str(yaml).unwrap();
        let archive = config
            .models
            .get("my_app")
            .unwrap()
            .archive
            .as_ref()
            .unwrap();

        assert_eq!(archive.includes, vec!["~/Desktop/test"]);
        assert_eq!(archive.excludes, vec!["~/Desktop/test/cache"]);
    }

    #[test]
    fn parse_archive_without_excludes() {
        let yaml = r#"
models:
  my_app:
    archive:
      includes:
        - ~/Desktop/test
"#;

        let config: Config = serde_yaml::from_str(yaml).unwrap();
        let archive = config
            .models
            .get("my_app")
            .unwrap()
            .archive
            .as_ref()
            .unwrap();

        assert_eq!(archive.includes, vec!["~/Desktop/test"]);
        assert!(archive.excludes.is_empty());
    }

    #[test]
    fn parse_config_without_archive() {
        let yaml = r#"
models:
  my_app:
    databases: {}
"#;

        let config: Config = serde_yaml::from_str(yaml).unwrap();
        let model = config.models.get("my_app").unwrap();
        assert!(model.archive.is_none());
    }

    #[test]
    fn parse_archive_without_includes() {
        let yaml = r#"
models:
  my_app:
    archive:
      excludes:
        - ~/Desktop/test/cache
"#;

        let config: Config = serde_yaml::from_str(yaml).unwrap();
        let archive = config
            .models
            .get("my_app")
            .unwrap()
            .archive
            .as_ref()
            .unwrap();

        assert!(archive.includes.is_empty());
        assert_eq!(archive.excludes, vec!["~/Desktop/test/cache"]);
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
