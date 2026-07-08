pub mod mysql;

use crate::database::mysql::MySQLConfig;
use serde::Deserialize;
use std::path::Path;

/// Configuration for a database — the `type` field determines which variant is used.
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
        }
    }
}

/// Run a database dump based on the configuration.
pub fn run(config: &DatabaseConfig, dump_path: &Path) -> Result<(), String> {
    config.as_dyn_database().perform(dump_path)
}
