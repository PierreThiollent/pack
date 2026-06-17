use chrono::{DateTime, Local, TimeZone};
use flate2::Compression;
use flate2::write::GzEncoder;
use serde::Deserialize;
use std::fmt::Display;
use std::fs::File;
use std::path::{Path, PathBuf};
use tracing::info;

/// Configuration for compression — the `type` field determines which variant is used.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum CompressorConfig {
    #[serde(rename = "tgz")]
    Tgz,
}

/// Prepare the artifact that will be uploaded by storages.
///
/// With `tgz`, the model dump directory is compressed into a `.tar.gz` file.
pub fn run(
    config: Option<&CompressorConfig>,
    dump_directory: &Path,
    model_name: &str,
) -> Result<PathBuf, String> {
    match config {
        Some(CompressorConfig::Tgz) => {
            let artifact_path = artifact_path(dump_directory, model_name, Local::now(), ".tar.gz")?;
            info!(
                "[Compressor: tgz] Creating compressed artifact: {}",
                artifact_path.display()
            );
            create_tgz(dump_directory, model_name, &artifact_path)?;
            info!(
                "[Compressor: tgz] Compressed artifact created: {}",
                artifact_path.display()
            );
            Ok(artifact_path)
        }
        None => {
            info!("[Compressor] No compression configured; using dump directory as artifact");
            Ok(dump_directory.to_path_buf())
        }
    }
}

/// Format the artifact timestamp so filenames are readable and sortable.
fn timestamp_label<Tz: TimeZone>(now: DateTime<Tz>) -> String
where
    Tz::Offset: Display,
{
    now.format("%Y%m%d-%H%M%S").to_string()
}

/// Build the compressed artifact filename from model name, timestamp and extension.
fn artifact_file_name(model_name: &str, timestamp: &str, extension: &str) -> String {
    format!("{model_name}-{timestamp}{extension}")
}

/// Build the artifact path next to the model dump directory.
fn artifact_path<Tz: TimeZone>(
    dump_directory: &Path,
    model_name: &str,
    now: DateTime<Tz>,
    extension: &str,
) -> Result<PathBuf, String>
where
    Tz::Offset: Display,
{
    let parent_directory = dump_directory.parent().ok_or_else(|| {
        format!("Failed to resolve artifact parent directory for {dump_directory:?}")
    })?;
    let file_name = artifact_file_name(model_name, &timestamp_label(now), extension);

    Ok(parent_directory.join(file_name))
}

/// Create a `.tar.gz` artifact containing the model dump directory.
fn create_tgz(dump_directory: &Path, model_name: &str, artifact_path: &Path) -> Result<(), String> {
    let artifact_file = File::create(artifact_path).map_err(|error| {
        format!("Failed to create compressed artifact {artifact_path:?}: {error}")
    })?;
    let encoder = GzEncoder::new(artifact_file, Compression::default());
    let mut builder = tar::Builder::new(encoder);

    builder
        .append_dir_all(model_name, dump_directory)
        .map_err(|error| {
            format!(
                "Failed to add dump directory {dump_directory:?} to compressed artifact: {error}"
            )
        })?;

    let encoder = builder
        .into_inner()
        .map_err(|error| format!("Failed to finish tar archive {artifact_path:?}: {error}"))?;
    encoder
        .finish()
        .map_err(|error| format!("Failed to finish gzip artifact {artifact_path:?}: {error}"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_file_name_uses_model_timestamp_and_extension() {
        let file_name = artifact_file_name("mon_site", "20260614-223510", ".tar.gz");

        assert_eq!(file_name, "mon_site-20260614-223510.tar.gz");
    }

    #[test]
    fn artifact_path_uses_run_directory_as_parent() {
        let dump_directory = Path::new("/tmp/pack-run/mon_site");
        let now = DateTime::parse_from_rfc3339("2026-06-14T22:35:10+02:00").unwrap();

        let path = artifact_path(dump_directory, "mon_site", now, ".tar.gz").unwrap();

        assert_eq!(
            path,
            PathBuf::from("/tmp/pack-run/mon_site-20260614-223510.tar.gz")
        );
    }

    #[test]
    fn timestamp_label_is_sortable_and_readable() {
        let now = DateTime::parse_from_rfc3339("2026-06-14T22:35:10+02:00").unwrap();

        let label = timestamp_label(now);

        assert_eq!(label, "20260614-223510");
    }

    #[test]
    fn run_without_compressor_returns_dump_directory() {
        let dump_directory = Path::new("/tmp/pack/model");

        let artifact_path = run(None, dump_directory, "mon_site").unwrap();

        assert_eq!(artifact_path, dump_directory);
    }

    #[test]
    fn run_with_tgz_creates_compressed_artifact() {
        let run_directory = tempfile::tempdir().unwrap();
        let dump_directory = run_directory.path().join("mon_site");
        std::fs::create_dir_all(&dump_directory).unwrap();
        std::fs::write(dump_directory.join("dump.sql"), "select 1;").unwrap();

        let artifact_path = run(Some(&CompressorConfig::Tgz), &dump_directory, "mon_site").unwrap();

        assert!(artifact_path.exists(), "Artifact should exist");
        assert!(
            artifact_path
                .file_name()
                .and_then(|file_name| file_name.to_str())
                .is_some_and(|file_name| file_name.ends_with(".tar.gz")),
            "Artifact should use .tar.gz extension: {artifact_path:?}"
        );

        let artifact_file = File::open(&artifact_path).unwrap();
        let decoder = flate2::read::GzDecoder::new(artifact_file);
        let mut archive = tar::Archive::new(decoder);
        let mut dump_sql_content = None;
        for entry in archive.entries().unwrap() {
            let mut entry = entry.unwrap();
            if entry.path().unwrap() == Path::new("mon_site/dump.sql") {
                let mut content = String::new();
                std::io::Read::read_to_string(&mut entry, &mut content).unwrap();
                dump_sql_content = Some(content);
            }
        }

        assert_eq!(dump_sql_content.as_deref(), Some("select 1;"));

        run_directory.close().unwrap();
    }
}
