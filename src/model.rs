use crate::config::Config;
use crate::config::Model;
use crate::database;
use std::path::PathBuf;

/// Run all backup models from the configuration.
///
/// For each model, creates a temporary dump directory, then runs
/// each configured database dump (MySQL, etc.) inside it.
/// The temporary directory is always cleaned up, even on failure.
pub fn run_all(config: &Config) -> Result<(), String> {
    for (name, model) in &config.models {
        println!("Model: {name}");

        let dump_dir = create_dump_dir(name)?;

        // Run the model and capture any error
        let result = run_model_databases(model, &dump_dir);

        // Always clean up, regardless of success or failure
        if let Err(e) = std::fs::remove_dir_all(&dump_dir) {
            eprintln!("Warning: failed to clean up dump directory {dump_dir:?}: {e}");
        }

        // Propagate the error after cleanup
        result?;
    }

    Ok(())
}

/// Run all database dumps for a single model inside the given dump directory.
fn run_model_databases(model: &Model, dump_dir: &PathBuf) -> Result<(), String> {
    for (db_name, db_config) in &model.databases {
        println!("  Database: {db_name}");
        database::run(db_config, &dump_dir.to_string_lossy())?;
        println!("  ✔ {db_name} done");
    }
    Ok(())
}

/// Create a temporary directory for a model's dump files.
///
/// Path: `{system_temp_dir}/rbak/{model_name}/`
fn create_dump_dir(model_name: &str) -> Result<PathBuf, String> {
    let mut dir = std::env::temp_dir();
    dir.push("rbak");
    dir.push(model_name);

    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create dump directory {dir:?}: {e}"))?;

    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, DatabaseConfig, Model, MySQLConfig};
    use std::collections::HashMap;

    fn make_config() -> Config {
        let mysql = MySQLConfig {
            host: "localhost".to_string(),
            port: None,
            database: "test".to_string(),
            username: Some("root".to_string()),
            password: Some("pass".to_string()),
        };

        let mut databases = HashMap::new();
        databases.insert("primary".to_string(), DatabaseConfig::MySQL(mysql));

        let mut models = HashMap::new();
        models.insert("my_app".to_string(), Model { databases });

        Config { models }
    }

    #[test]
    fn run_all_creates_and_cleans_dump_dir() {
        let config = make_config();

        let result = run_all(&config);

        // mysqldump will fail (no server running), but that's expected.
        // The important thing is that the dump directory was cleaned up.
        assert!(result.is_err());

        // Verify the dump directory no longer exists
        let mut dump_dir = std::env::temp_dir();
        dump_dir.push("rbak");
        dump_dir.push("my_app");
        assert!(!dump_dir.exists(), "Dump directory should be cleaned up");
    }

    #[test]
    fn create_dump_dir_creates_directory() {
        let dir = create_dump_dir("test_model").unwrap();
        assert!(dir.exists(), "Directory should exist");
        assert!(dir.is_dir(), "Path should be a directory");

        // Cleanup
        if let Err(error) = std::fs::remove_dir_all(&dir) {
            panic!("Failed to clean up test directory {dir:?}: {error}");
        }
    }
}
