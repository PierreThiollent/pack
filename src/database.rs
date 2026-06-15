pub mod mysql;

use crate::database::mysql::{MySQL, MySQLConfig};
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

/// Run a database dump based on the configuration.
///
/// Dispatches to the correct database implementation (MySQL, PostgreSQL, …).
pub fn run(config: &DatabaseConfig, dump_path: &Path) -> Result<(), String> {
    match config {
        DatabaseConfig::MySQL(mysql_config) => {
            let mysql = MySQL::new(mysql_config, dump_path);
            mysql.perform()
        }
    }
}
