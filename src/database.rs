pub mod mysql;
pub mod postgresql;

use crate::database::mysql::MySQLConfig;
use crate::database::postgresql::PostgreSQLConfig;
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Configuration for a database — the `type` field determines which variant is used.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum DatabaseConfig {
    #[serde(rename = "mysql")]
    MySQL(MySQLConfig),

    #[serde(rename = "postgresql")]
    PostgreSQL(PostgreSQLConfig),
}

impl DatabaseConfig {
    pub fn type_name(&self) -> &'static str {
        match self {
            DatabaseConfig::MySQL(_) => "MySQL",
            DatabaseConfig::PostgreSQL(_) => "PostgreSQL",
        }
    }
}

/// Common behavior required from every database backend.
///
/// The enum remains responsible for YAML parsing, while each concrete config
/// owns its dump behavior.
pub(crate) trait Database {
    fn perform(&self, dump_path: &Path) -> Result<(), String>;
}

impl DatabaseConfig {
    /// Return this enum variant as the common database trait object.
    ///
    /// New database types should only need to extend this dispatch point, then
    /// implement `Database` in their own module.
    fn as_dyn_database(&self) -> &dyn Database {
        match self {
            DatabaseConfig::MySQL(config) => config,
            DatabaseConfig::PostgreSQL(config) => config,
        }
    }
}

/// Run a database dump based on the configuration.
pub fn run(config: &DatabaseConfig, dump_path: &Path) -> Result<(), String> {
    config.as_dyn_database().perform(dump_path)
}

/// Build a safe SQL dump file path from a database name.
///
/// Database names come from the YAML config and are used to create local files.
/// Reject path separators and special path components instead of silently
/// writing outside the model dump directory.
pub(crate) fn dump_file_path(dump_path: &Path, database_name: &str) -> Result<PathBuf, String> {
    validate_dump_file_name(database_name)?;
    Ok(dump_path.join(format!("{database_name}.sql")))
}

fn validate_dump_file_name(database_name: &str) -> Result<(), String> {
    if database_name.trim().is_empty()
        || database_name == "."
        || database_name == ".."
        || database_name.contains('/')
        || database_name.contains('\\')
    {
        return Err(format!(
            "Invalid database name for dump file: {database_name}"
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dump_file_path_uses_database_name() {
        let path = dump_file_path(Path::new("/tmp/dumps"), "my_app").unwrap();

        assert_eq!(path, Path::new("/tmp/dumps/my_app.sql"));
    }

    #[test]
    fn dump_file_path_accepts_common_database_name_characters() {
        let path = dump_file_path(Path::new("/tmp/dumps"), "my-app.prod_01").unwrap();

        assert_eq!(path, Path::new("/tmp/dumps/my-app.prod_01.sql"));
    }

    #[test]
    fn dump_file_path_rejects_unsafe_database_names() {
        for database_name in [
            "",
            "   ",
            ".",
            "..",
            "../escaped",
            "nested/db",
            "nested\\db",
        ] {
            assert!(dump_file_path(Path::new("/tmp/dumps"), database_name).is_err());
        }
    }
}
