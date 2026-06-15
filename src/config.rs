use crate::archive::ArchiveConfig;
use crate::compressor::CompressorConfig;
use crate::database::DatabaseConfig;
use crate::paths;
use crate::storage::StorageConfig;
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

    /// Compression configuration for the backup artifact
    pub compress_with: Option<CompressorConfig>,
}

/// Resolve the config file path.
/// If `config_arg` is `Some`, use it (with tilde expansion).
/// Otherwise, default to `$HOME/.pack/pack.yml`.
pub fn resolve_config_path(config_arg: Option<String>) -> String {
    let path = config_arg.unwrap_or_else(|| format!("{}/.pack/pack.yml", paths::home_dir()));
    paths::expand_tilde(&path)
}

pub(crate) fn validate_model_name(model_name: &str) -> Result<(), String> {
    if model_name.is_empty() {
        return Err("Model name cannot be empty".to_string());
    }

    if !model_name
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
    {
        return Err(format!(
            "Invalid model name `{model_name}`: use only ASCII letters, digits, `_` and `-`"
        ));
    }

    Ok(())
}

/// Load and parse the config file from the given path.
/// Exits the process with an error message on failure.
pub fn load_config(path: &str) -> Config {
    let yaml_content = std::fs::read_to_string(path).unwrap_or_else(|error| {
        error!("[Config] Failed to read config file {path}: {error}");
        std::process::exit(1);
    });
    let config: Config = serde_yaml::from_str(&yaml_content).unwrap_or_else(|error| {
        error!("[Config] Failed to parse config file {path}: {error}");
        std::process::exit(1);
    });

    validate_config(&config).unwrap_or_else(|error| {
        error!("[Config] Invalid config file {path}: {error}");
        std::process::exit(1);
    });

    config
}

fn validate_config(config: &Config) -> Result<(), String> {
    for model_name in config.models.keys() {
        validate_model_name(model_name)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_config_with_workdir() {
        let yaml = r#"
workdir: /var/tmp/pack
models: {}
"#;

        let config: Config = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(config.workdir.as_deref(), Some("/var/tmp/pack"));
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
        path: ~/Desktop/pack-backups
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
                assert_eq!(local_config.path, "~/Desktop/pack-backups");
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
    fn parse_compressor_tgz() {
        let yaml = r#"
models:
  my_app:
    compress_with:
      type: tgz
"#;

        let config: Config = serde_yaml::from_str(yaml).unwrap();
        let compressor = config
            .models
            .get("my_app")
            .unwrap()
            .compress_with
            .as_ref()
            .unwrap();

        assert!(matches!(compressor, CompressorConfig::Tgz));
    }

    #[test]
    fn parse_config_without_compressor() {
        let yaml = r#"
models:
  my_app:
    databases: {}
"#;

        let config: Config = serde_yaml::from_str(yaml).unwrap();
        let model = config.models.get("my_app").unwrap();
        assert!(model.compress_with.is_none());
    }

    #[test]
    fn parse_invalid_yaml_unknown_compressor_type() {
        let yaml = r#"
models:
  my_app:
    compress_with:
      type: zip
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
    fn validate_config_accepts_safe_model_names() {
        let yaml = r#"
models:
  my_app:
    databases: {}
  wordpress-prod:
    databases: {}
"#;

        let config: Config = serde_yaml::from_str(yaml).unwrap();

        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn validate_config_rejects_unsafe_model_names() {
        let yaml = r#"
models:
  ../escaped:
    databases: {}
"#;

        let config: Config = serde_yaml::from_str(yaml).unwrap();
        let error = validate_config(&config).unwrap_err();

        assert!(error.contains("Invalid model name"));
    }

    #[test]
    fn validate_model_name_accepts_safe_names() {
        for model_name in ["my_app", "wordpress-prod", "client42"] {
            assert!(
                validate_model_name(model_name).is_ok(),
                "Model name should be accepted: {model_name}"
            );
        }
    }

    #[test]
    fn validate_model_name_rejects_unsafe_names() {
        for model_name in [
            "", "../foo", "foo/bar", "foo\\bar", ".hidden", "my.app", "my app",
        ] {
            assert!(
                validate_model_name(model_name).is_err(),
                "Model name should be rejected: {model_name}"
            );
        }
    }

    #[test]
    fn resolve_default_config_path() {
        let home = std::env::var("HOME").unwrap();
        let path = resolve_config_path(None);
        assert_eq!(path, format!("{home}/.pack/pack.yml"));
    }

    #[test]
    fn resolve_config_path_with_tilde() {
        let home = std::env::var("HOME").unwrap();
        let path = resolve_config_path(Some("~/custom/path.yml".to_string()));
        assert_eq!(path, format!("{home}/custom/path.yml"));
    }

    #[test]
    fn resolve_config_path_absolute() {
        let path = resolve_config_path(Some("/etc/pack.yml".to_string()));
        assert_eq!(path, "/etc/pack.yml");
    }

    #[test]
    fn resolve_config_path_relative() {
        let path = resolve_config_path(Some("./local.yml".to_string()));
        assert_eq!(path, "./local.yml");
    }
}
