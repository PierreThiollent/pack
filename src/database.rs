pub mod mysql;

use crate::config::DatabaseConfig;
use mysql::MySQL;

/// Run a database dump based on the configuration.
///
/// Dispatches to the correct database implementation (MySQL, PostgreSQL, …).
pub fn run(config: &DatabaseConfig, dump_path: &str) -> Result<(), String> {
    match config {
        DatabaseConfig::MySQL(mysql_config) => {
            let mysql = MySQL::new(mysql_config, dump_path);
            mysql.perform()
        }
    }
}
