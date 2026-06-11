use crate::config::ArchiveConfig;
use std::path::{Path, PathBuf};

const ARCHIVE_FILE_NAME: &str = "archive.tar";

/// Create an archive from configured files and directories.
///
/// The actual tar creation will be implemented in the next step. For now,
/// this validates the configuration and centralizes the archive output path.
pub fn run(config: Option<&ArchiveConfig>, dump_directory: &Path) -> Result<(), String> {
    let Some(config) = config else {
        return Ok(());
    };

    if config.includes.is_empty() {
        return Err("archive.includes cannot be empty".to_string());
    }

    let archive_path = archive_path(dump_directory);
    println!(
        "  Archive: {} include(s), {} exclude(s) -> {}",
        config.includes.len(),
        config.excludes.len(),
        archive_path.display()
    );

    Ok(())
}

fn archive_path(dump_directory: &Path) -> PathBuf {
    dump_directory.join(ARCHIVE_FILE_NAME)
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
        let dump_directory = Path::new("/tmp/rbak/model");

        let result = archive_path(dump_directory);

        assert_eq!(result, PathBuf::from("/tmp/rbak/model/archive.tar"));
    }

    #[test]
    fn run_without_archive_config_succeeds() {
        let dump_directory = Path::new("/tmp/rbak/model");

        let result = run(None, dump_directory);

        assert!(result.is_ok());
    }

    #[test]
    fn run_with_empty_includes_returns_error() {
        let config = make_config(vec![], vec!["~/Desktop/test/cache"]);
        let dump_directory = Path::new("/tmp/rbak/model");

        let result = run(Some(&config), dump_directory);

        assert!(result.is_err());
    }

    #[test]
    fn run_with_includes_succeeds() {
        let config = make_config(vec!["~/Desktop/test"], vec!["~/Desktop/test/cache"]);
        let dump_directory = Path::new("/tmp/rbak/model");

        let result = run(Some(&config), dump_directory);

        assert!(result.is_ok());
    }
}
