use std::collections::HashMap;

use serde::Deserialize;

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

/// Configuration for a single database
#[derive(Debug, Deserialize)]
pub struct DatabaseConfig {
    /// Database type (e.g. "mysql", "postgresql", "redis")
    #[serde(rename = "type")]
    pub db_type: String,
    #[serde(default = "default_host")]
    pub host: String,
    pub port: Option<u16>,
    pub database: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
}

fn default_host() -> String {
    "localhost".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_config_with_one_model() {
        let yaml = r#"
models:
  my_app:
    databases:
      my_db:
        type: mysql
"#;

        let config: Config = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(config.models.len(), 1);

        let model = config.models.get("my_app").unwrap();
        assert_eq!(model.databases.len(), 1);

        let db = model.databases.get("my_db").unwrap();
        assert_eq!(db.db_type, "mysql");
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
    fn parse_mysql_config_with_all_fields() {
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
        let db = &config.models.get("my_app").unwrap().databases["my_db"];

        assert_eq!(db.db_type, "mysql");
        assert_eq!(db.host, "db.example.com");
        assert_eq!(db.port, Some(3307));
        assert_eq!(db.database.as_deref(), Some("my_production_db"));
        assert_eq!(db.username.as_deref(), Some("root"));
        assert_eq!(db.password.as_deref(), Some("secret123"));
    }

    #[test]
    fn parse_mysql_config_with_default_host() {
        let yaml = r#"
models:
  my_app:
    databases:
      my_db:
        type: mysql
"#;

        let config: Config = serde_yaml::from_str(yaml).unwrap();
        let db = &config.models.get("my_app").unwrap().databases["my_db"];

        assert_eq!(db.host, "localhost");
        assert_eq!(db.port, None);
        assert_eq!(db.database, None);
    }

    #[test]
    fn parse_invalid_yaml_missing_field() {
        let yaml = r#"
foo: bar
"#;

        let result = serde_yaml::from_str::<Config>(yaml);

        assert!(result.is_err());
    }
}
