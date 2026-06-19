use crate::paths;
use serde::Deserialize;
use std::fs::File;
use std::path::{Component, Path, PathBuf};
use tracing::{info, warn};

const ARCHIVE_FILE_NAME: &str = "archive.tar";
const ARCHIVE_ROOT_DIRECTORY: &str = "archive";

/// Configuration for additional files and directories to archive.
#[derive(Debug, Deserialize)]
pub struct ArchiveConfig {
    #[serde(default)]
    pub includes: Vec<String>,

    #[serde(default)]
    pub excludes: Vec<String>,
}

/// Create an archive from configured files and directories.
pub fn run(config: Option<&ArchiveConfig>, dump_directory: &Path) -> Result<(), String> {
    let Some(config) = config else {
        return Ok(());
    };

    if config.includes.is_empty() {
        return Err("archive.includes cannot be empty".to_string());
    }

    let archive_path = archive_path(dump_directory);
    info!(
        "[Archive] Creating archive: {} include(s), {} exclude(s)",
        config.includes.len(),
        config.excludes.len()
    );

    create_archive(config, &archive_path)?;

    info!("[Archive] Archive created: {}", archive_path.display());

    Ok(())
}

/// Return the path of the intermediate `archive.tar` inside the dump directory.
fn archive_path(dump_directory: &Path) -> PathBuf {
    dump_directory.join(ARCHIVE_FILE_NAME)
}

/// Create `archive.tar` and append every configured include to it.
fn create_archive(config: &ArchiveConfig, archive_path: &Path) -> Result<(), String> {
    if let Some(parent_directory) = archive_path.parent() {
        std::fs::create_dir_all(parent_directory).map_err(|error| {
            format!("Failed to create archive directory {parent_directory:?}: {error}")
        })?;
    }

    let archive_file = File::create(archive_path)
        .map_err(|error| format!("Failed to create archive file {archive_path:?}: {error}"))?;
    let mut builder = tar::Builder::new(archive_file);
    let excludes = expanded_excludes(config);

    for include in &config.includes {
        append_include(&mut builder, include, &excludes)?;
    }

    builder
        .finish()
        .map_err(|error| format!("Failed to finish archive {archive_path:?}: {error}"))?;

    Ok(())
}

/// Add one configured include to the archive.
fn append_include(
    builder: &mut tar::Builder<File>,
    include: &str,
    excludes: &[PathBuf],
) -> Result<(), String> {
    let source_path = PathBuf::from(paths::expand_tilde(include));

    if !source_path.exists() {
        return Err(format!("Archive include does not exist: {source_path:?}"));
    }

    append_path(builder, &source_path, excludes)
}

/// Add a file or directory to the archive, dispatching by source path type.
fn append_path(
    builder: &mut tar::Builder<File>,
    source_path: &Path,
    excludes: &[PathBuf],
) -> Result<(), String> {
    if is_excluded(source_path, excludes) {
        info!("[Archive] Excluding path: {}", source_path.display());
        return Ok(());
    }

    if source_path.is_dir() {
        append_directory(builder, source_path, excludes)
    } else {
        append_file(builder, source_path)
    }
}

/// Recursively add a directory's entries to the archive.
fn append_directory(
    builder: &mut tar::Builder<File>,
    source_path: &Path,
    excludes: &[PathBuf],
) -> Result<(), String> {
    let entries = match std::fs::read_dir(source_path) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            warn!(
                "[Archive] Skipping vanished directory during archive: {}",
                source_path.display()
            );
            return Ok(());
        }
        Err(error) => {
            return Err(format!(
                "Failed to read archive directory {source_path:?}: {error}"
            ));
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                warn!(
                    "[Archive] Skipping vanished directory entry during archive in: {}",
                    source_path.display()
                );
                continue;
            }
            Err(error) => {
                return Err(format!(
                    "Failed to read archive directory entry in {source_path:?}: {error}"
                ));
            }
        };
        append_path(builder, &entry.path(), excludes)?;
    }

    Ok(())
}

fn expanded_excludes(config: &ArchiveConfig) -> Vec<PathBuf> {
    config
        .excludes
        .iter()
        .map(|exclude| PathBuf::from(paths::expand_tilde(exclude)))
        .collect()
}

fn is_excluded(source_path: &Path, excludes: &[PathBuf]) -> bool {
    excludes
        .iter()
        .any(|exclude| source_path == exclude || source_path.starts_with(exclude))
}

/// Add a single file to the archive under its normalized archive entry path.
fn append_file(builder: &mut tar::Builder<File>, source_path: &Path) -> Result<(), String> {
    let archive_entry_path = archive_entry_path(source_path);
    match builder.append_path_with_name(source_path, &archive_entry_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            warn!(
                "[Archive] Skipping vanished file during archive: {}",
                source_path.display()
            );
            Ok(())
        }
        Err(error) => Err(format!(
            "Failed to add archive include {source_path:?} as {archive_entry_path:?}: {error}"
        )),
    }
}

/// Build the path used inside `archive.tar` for a source path.
fn archive_entry_path(source_path: &Path) -> PathBuf {
    let mut entry_path = PathBuf::from(ARCHIVE_ROOT_DIRECTORY);

    for component in source_path.components() {
        match component {
            Component::Normal(part) => entry_path.push(part),
            Component::CurDir => {}
            Component::ParentDir => entry_path.push("__parent__"),
            Component::RootDir | Component::Prefix(_) => {}
        }
    }

    entry_path
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(includes: Vec<&str>, excludes: Vec<&str>) -> ArchiveConfig {
        ArchiveConfig {
            includes: includes.into_iter().map(String::from).collect(),
            excludes: excludes.into_iter().map(String::from).collect(),
        }
    }

    #[test]
    fn archive_path_returns_archive_tar_inside_dump_directory() {
        let dump_directory = Path::new("/tmp/pack/model");

        let result = archive_path(dump_directory);

        assert_eq!(result, PathBuf::from("/tmp/pack/model/archive.tar"));
    }

    #[test]
    fn archive_entry_path_prefixes_absolute_path_with_archive_directory() {
        let source_path = Path::new("/tmp/pack/config.yml");

        let result = archive_entry_path(source_path);

        assert_eq!(result, PathBuf::from("archive/tmp/pack/config.yml"));
    }

    #[test]
    fn archive_entry_path_prefixes_relative_path_with_archive_directory() {
        let source_path = Path::new("config/app.yml");

        let result = archive_entry_path(source_path);

        assert_eq!(result, PathBuf::from("archive/config/app.yml"));
    }

    #[test]
    fn run_without_archive_config_succeeds() {
        let dump_directory = tempfile::tempdir().unwrap();

        let result = run(None, dump_directory.path());

        assert!(result.is_ok());
    }

    #[test]
    fn run_with_empty_includes_returns_error() {
        let config = make_config(vec![], vec!["~/Desktop/test/cache"]);
        let dump_directory = tempfile::tempdir().unwrap();

        let result = run(Some(&config), dump_directory.path());

        assert!(result.is_err());
    }

    #[test]
    fn run_with_missing_include_returns_error() {
        let config = make_config(vec!["/path/that/does/not/exist"], vec![]);
        let dump_directory = tempfile::tempdir().unwrap();

        let result = run(Some(&config), dump_directory.path());

        assert!(result.is_err());
    }

    fn archive_entry_paths(dump_directory: &Path) -> Vec<PathBuf> {
        let archive_file = File::open(dump_directory.join(ARCHIVE_FILE_NAME)).unwrap();
        let mut archive = tar::Archive::new(archive_file);
        let mut entry_paths = Vec::new();
        for entry in archive.entries().unwrap() {
            let entry = entry.unwrap();
            entry_paths.push(entry.path().unwrap().into_owned());
        }
        entry_paths
    }

    #[test]
    fn run_with_directory_include_stores_directory_files_in_archive() {
        let source_directory = tempfile::tempdir().unwrap();
        let dump_directory = tempfile::tempdir().unwrap();
        std::fs::write(source_directory.path().join("config.yml"), "hello archive").unwrap();
        let source_directory_string = source_directory.path().to_string_lossy();
        let config = make_config(vec![&source_directory_string], vec![]);

        let result = run(Some(&config), dump_directory.path());

        assert!(result.is_ok());
        let entry_paths = archive_entry_paths(dump_directory.path());
        assert!(entry_paths.iter().any(|path| path.ends_with("config.yml")));
    }

    #[test]
    fn run_with_directory_include_stores_nested_files_in_archive() {
        let source_directory = tempfile::tempdir().unwrap();
        let dump_directory = tempfile::tempdir().unwrap();
        let nested_directory = source_directory.path().join("nested");
        std::fs::create_dir_all(&nested_directory).unwrap();
        std::fs::write(nested_directory.join("config.yml"), "nested archive").unwrap();
        let source_directory_string = source_directory.path().to_string_lossy();
        let config = make_config(vec![&source_directory_string], vec![]);

        let result = run(Some(&config), dump_directory.path());

        assert!(result.is_ok());
        let entry_paths = archive_entry_paths(dump_directory.path());
        assert!(
            entry_paths
                .iter()
                .any(|path| path.ends_with("nested/config.yml"))
        );
    }

    #[test]
    fn run_with_directory_exclude_skips_excluded_directory_files() {
        let source_directory = tempfile::tempdir().unwrap();
        let dump_directory = tempfile::tempdir().unwrap();
        let cache_directory = source_directory.path().join("cache");
        std::fs::create_dir_all(&cache_directory).unwrap();
        std::fs::write(source_directory.path().join("config.yml"), "hello archive").unwrap();
        std::fs::write(cache_directory.join("temporary.txt"), "temporary cache").unwrap();
        let source_directory_string = source_directory.path().to_string_lossy();
        let cache_directory_string = cache_directory.to_string_lossy();
        let config = make_config(
            vec![&source_directory_string],
            vec![&cache_directory_string],
        );

        let result = run(Some(&config), dump_directory.path());

        assert!(result.is_ok());
        let entry_paths = archive_entry_paths(dump_directory.path());
        assert!(entry_paths.iter().any(|path| path.ends_with("config.yml")));
        assert!(
            !entry_paths
                .iter()
                .any(|path| path.ends_with("cache/temporary.txt"))
        );
    }

    #[test]
    fn append_file_skips_file_that_vanished_during_archive() {
        let source_directory = tempfile::tempdir().unwrap();
        let dump_directory = tempfile::tempdir().unwrap();
        let archive_file = File::create(dump_directory.path().join(ARCHIVE_FILE_NAME)).unwrap();
        let mut builder = tar::Builder::new(archive_file);
        let source_file = source_directory.path().join("cache.tmp");
        std::fs::write(&source_file, "temporary cache").unwrap();
        std::fs::remove_file(&source_file).unwrap();

        let result = append_file(&mut builder, &source_file);

        assert!(result.is_ok());
    }

    #[test]
    fn append_directory_skips_directory_that_vanished_during_archive() {
        let source_directory = tempfile::tempdir().unwrap();
        let dump_directory = tempfile::tempdir().unwrap();
        let archive_file = File::create(dump_directory.path().join(ARCHIVE_FILE_NAME)).unwrap();
        let mut builder = tar::Builder::new(archive_file);
        let vanished_directory = source_directory.path().join("cache");
        std::fs::create_dir(&vanished_directory).unwrap();
        std::fs::remove_dir(&vanished_directory).unwrap();

        let result = append_directory(&mut builder, &vanished_directory, &[]);

        assert!(result.is_ok());
    }

    #[test]
    fn is_excluded_matches_exact_path() {
        let excludes = vec![PathBuf::from("/tmp/app/cache")];

        assert!(is_excluded(Path::new("/tmp/app/cache"), &excludes));
    }

    #[test]
    fn is_excluded_matches_path_inside_excluded_directory() {
        let excludes = vec![PathBuf::from("/tmp/app/cache")];

        assert!(is_excluded(Path::new("/tmp/app/cache/file.txt"), &excludes));
    }

    #[test]
    fn is_excluded_does_not_match_path_with_similar_prefix() {
        let excludes = vec![PathBuf::from("/tmp/app/cache")];

        assert!(!is_excluded(
            Path::new("/tmp/app/cache-old/file.txt"),
            &excludes
        ));
    }

    #[test]
    fn run_with_file_include_creates_archive_tar() {
        let source_directory = tempfile::tempdir().unwrap();
        let dump_directory = tempfile::tempdir().unwrap();
        let source_file = source_directory.path().join("config.yml");
        std::fs::write(&source_file, "hello archive").unwrap();
        let source_file_string = source_file.to_string_lossy();
        let config = make_config(vec![&source_file_string], vec![]);

        let result = run(Some(&config), dump_directory.path());

        assert!(result.is_ok());
        assert!(dump_directory.path().join(ARCHIVE_FILE_NAME).exists());
    }

    #[test]
    fn run_with_file_include_stores_file_in_archive() {
        let source_directory = tempfile::tempdir().unwrap();
        let dump_directory = tempfile::tempdir().unwrap();
        let source_file = source_directory.path().join("config.yml");
        std::fs::write(&source_file, "hello archive").unwrap();
        let source_file_string = source_file.to_string_lossy();
        let config = make_config(vec![&source_file_string], vec![]);

        run(Some(&config), dump_directory.path()).unwrap();

        let entry_paths = archive_entry_paths(dump_directory.path());

        assert!(entry_paths.iter().any(|path| path.ends_with("config.yml")));
    }
}
