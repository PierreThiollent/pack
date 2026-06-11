use crate::archive;
use crate::config::{Config, Model};
use crate::database;
use crate::paths;
use crate::storage;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::{Builder, TempDir};

/// Run all backup models from the configuration.
///
/// A unique temporary run directory is created first, then each model gets
/// its own dump directory inside it. The run directory is always cleaned up,
/// even on failure.
pub fn run_all(config: &Config) -> Result<(), String> {
    let run_directory = create_run_directory(config.workdir.as_deref())?;
    println!("Run directory: {}", run_directory.path().display());

    let result = run_models(config, run_directory.path());

    let run_directory_path = run_directory.path().to_path_buf();
    if let Err(error) = run_directory.close() {
        eprintln!("Warning: failed to clean up run directory {run_directory_path:?}: {error}");
    }

    result
}

fn run_models(config: &Config, run_directory: &Path) -> Result<(), String> {
    for (name, model) in &config.models {
        println!("Model: {name}");

        let dump_directory = create_dump_directory(run_directory, name)?;
        run_model_databases(model, &dump_directory)?;
        archive::run(model.archive.as_ref(), &dump_directory)?;
        run_model_storages(model, &dump_directory)?;
    }

    Ok(())
}

/// Run all database dumps for a single model inside the given dump directory.
fn run_model_databases(model: &Model, dump_directory: &Path) -> Result<(), String> {
    for (database_name, database_config) in &model.databases {
        println!("  Database: {database_name}");
        database::run(database_config, &dump_directory.to_string_lossy())?;
        println!("  ✔ {database_name} done");
    }
    Ok(())
}

fn run_model_storages(model: &Model, source_path: &Path) -> Result<(), String> {
    for (storage_name, storage_config) in &model.storages {
        println!("  Storage: {storage_name}");
        storage::run(storage_config, source_path)?;
        println!("  ✔ {storage_name} done");
    }
    Ok(())
}

/// Create a unique temporary directory for one `perform` run.
///
/// Path: `{workdir_or_system_temp_dir}/rbak-{timestamp}-{random}/`
fn create_run_directory(workdir: Option<&str>) -> Result<TempDir, String> {
    let root_directory = match workdir {
        Some(path) if !path.trim().is_empty() => PathBuf::from(paths::expand_tilde(path)),
        _ => std::env::temp_dir(),
    };

    std::fs::create_dir_all(&root_directory)
        .map_err(|error| format!("Failed to create workdir {root_directory:?}: {error}"))?;

    let timestamp = current_timestamp_seconds()?;
    let prefix = format!("rbak-{timestamp}-");

    Builder::new()
        .prefix(&prefix)
        .tempdir_in(&root_directory)
        .map_err(|error| format!("Failed to create run directory in {root_directory:?}: {error}"))
}

/// Create a temporary directory for a model's dump files.
///
/// Path: `{run_directory}/{model_name}/`
fn create_dump_directory(run_directory: &Path, model_name: &str) -> Result<PathBuf, String> {
    let directory = run_directory.join(model_name);

    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("Failed to create dump directory {directory:?}: {error}"))?;

    Ok(directory)
}

fn current_timestamp_seconds() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| format!("System clock is before UNIX epoch: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::collections::HashMap;

    fn make_config(workdir: Option<String>) -> Config {
        let mut models = HashMap::new();
        models.insert(
            "my_app".to_string(),
            Model {
                databases: HashMap::new(),
                storages: HashMap::new(),
                archive: None,
            },
        );

        Config { workdir, models }
    }

    fn assert_directory_is_empty(path: &Path) {
        let entries: Result<Vec<_>, _> = std::fs::read_dir(path).unwrap().collect();
        let entries = entries.unwrap();

        assert!(entries.is_empty(), "Directory should be empty: {path:?}");
    }

    #[test]
    fn run_all_creates_and_cleans_run_directory() {
        let workdir = tempfile::tempdir().unwrap();
        let config = make_config(Some(workdir.path().to_string_lossy().into_owned()));

        let result = run_all(&config);

        assert!(result.is_ok());
        assert_directory_is_empty(workdir.path());

        workdir.close().unwrap();
    }

    #[test]
    fn create_run_directory_creates_unique_directory() {
        let first_run_directory = create_run_directory(None).unwrap();
        let second_run_directory = create_run_directory(None).unwrap();

        assert!(
            first_run_directory.path().exists(),
            "Directory should exist"
        );
        assert!(
            second_run_directory.path().exists(),
            "Directory should exist"
        );
        assert_ne!(first_run_directory.path(), second_run_directory.path());

        let first_run_directory_path = first_run_directory.path().to_path_buf();
        let second_run_directory_path = second_run_directory.path().to_path_buf();

        first_run_directory.close().unwrap();
        second_run_directory.close().unwrap();

        assert!(!first_run_directory_path.exists());
        assert!(!second_run_directory_path.exists());
    }

    #[test]
    fn create_run_directory_uses_system_temp_directory_when_workdir_is_empty() {
        let run_directory = create_run_directory(Some("")).unwrap();
        let run_directory_path = run_directory.path().to_path_buf();

        assert!(run_directory_path.starts_with(std::env::temp_dir()));

        run_directory.close().unwrap();
    }

    #[test]
    fn create_run_directory_uses_configured_workdir() {
        let workdir = tempfile::tempdir().unwrap();
        let workdir_string = workdir.path().to_string_lossy().into_owned();

        let run_directory = create_run_directory(Some(&workdir_string)).unwrap();

        assert_eq!(run_directory.path().parent().unwrap(), workdir.path());
        assert!(run_directory.path().exists(), "Directory should exist");

        run_directory.close().unwrap();
        assert_directory_is_empty(workdir.path());

        workdir.close().unwrap();
    }

    #[test]
    fn create_run_directory_creates_missing_workdir() {
        let parent_directory = tempfile::tempdir().unwrap();
        let workdir = parent_directory.path().join("missing-workdir");
        let workdir_string = workdir.to_string_lossy().into_owned();

        let run_directory = create_run_directory(Some(&workdir_string)).unwrap();

        assert!(workdir.exists(), "Workdir should exist");
        assert_eq!(run_directory.path().parent().unwrap(), workdir.as_path());

        run_directory.close().unwrap();
        parent_directory.close().unwrap();
    }

    #[test]
    fn create_run_directory_expands_tilde_in_workdir() {
        let home = std::env::var("HOME").unwrap();
        let timestamp = current_timestamp_seconds().unwrap();
        let workdir_name = format!("rbak-tilde-workdir-test-{}-{timestamp}", std::process::id());
        let configured_workdir = format!("~/{workdir_name}");
        let expected_workdir = PathBuf::from(home).join(&workdir_name);

        let run_directory = create_run_directory(Some(&configured_workdir)).unwrap();

        assert_eq!(
            run_directory.path().parent().unwrap(),
            expected_workdir.as_path()
        );
        assert!(run_directory.path().exists(), "Directory should exist");

        run_directory.close().unwrap();

        if let Err(error) = std::fs::remove_dir_all(&expected_workdir) {
            panic!("Failed to clean up test workdir {expected_workdir:?}: {error}");
        }
    }

    #[test]
    fn create_dump_directory_creates_model_directory_inside_run_directory() {
        let run_directory = tempfile::tempdir().unwrap();

        let dump_directory = create_dump_directory(run_directory.path(), "test_model").unwrap();

        assert_eq!(dump_directory, run_directory.path().join("test_model"));
        assert!(dump_directory.exists(), "Directory should exist");
        assert!(dump_directory.is_dir(), "Path should be a directory");

        run_directory.close().unwrap();
    }
}
