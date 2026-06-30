use crate::logging::{LogTag, tag};
use chrono::{Local, NaiveDateTime};
use gzp::ZWriter;
use gzp::deflate::Gzip;
use gzp::par::compress::{Compression, ParCompress, ParCompressBuilder};
use serde::Deserialize;
use std::fs::File;
use std::path::{Path, PathBuf};
use tracing::info;

const TGZ_COMPRESSION_LEVEL: u32 = 4;

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
            let timestamp = timestamp_label(Local::now().naive_local());
            let artifact_path = artifact_path(dump_directory, model_name, &timestamp, ".tar.gz")?;
            info!(
                pack_tag = %tag(LogTag::Compressor(Some("tgz"))),
                "Creating compressed artifact: {}",
                artifact_path.display()
            );
            create_tgz(dump_directory, model_name, &artifact_path)?;
            info!(
                pack_tag = %tag(LogTag::Compressor(Some("tgz"))),
                "Compressed artifact created: {}",
                artifact_path.display()
            );
            Ok(artifact_path)
        }
        None => {
            info!(
                pack_tag = %tag(LogTag::Compressor(None)),
                "No compression configured; using dump directory as artifact"
            );
            Ok(dump_directory.to_path_buf())
        }
    }
}

/// Format the artifact timestamp so filenames are readable and sortable.
fn timestamp_label(now: NaiveDateTime) -> String {
    now.format("%Y%m%d-%H%M%S").to_string()
}

/// Build the compressed artifact filename from model name, timestamp and extension.
fn artifact_file_name(model_name: &str, timestamp: &str, extension: &str) -> String {
    format!("{model_name}-{timestamp}{extension}")
}

/// Build the artifact path next to the model dump directory.
fn artifact_path(
    dump_directory: &Path,
    model_name: &str,
    timestamp: &str,
    extension: &str,
) -> Result<PathBuf, String> {
    let parent_directory = dump_directory.parent().ok_or_else(|| {
        format!("Failed to resolve artifact parent directory for {dump_directory:?}")
    })?;
    let file_name = artifact_file_name(model_name, timestamp, extension);

    Ok(parent_directory.join(file_name))
}

/// Create a `.tar.gz` artifact containing the model dump directory.
fn create_tgz(dump_directory: &Path, model_name: &str, artifact_path: &Path) -> Result<(), String> {
    let artifact_file = File::create(artifact_path).map_err(|error| {
        format!("Failed to create compressed artifact {artifact_path:?}: {error}")
    })?;
    let encoder = create_gzip_encoder(artifact_file);
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
    finish_gzip_encoder(encoder, artifact_path)?;

    Ok(())
}

fn create_gzip_encoder(output_file: File) -> ParCompress<'static, Gzip, File> {
    ParCompressBuilder::new()
        .compression_level(Compression::new(TGZ_COMPRESSION_LEVEL))
        .from_writer(output_file)
}

fn finish_gzip_encoder(
    mut encoder: ParCompress<'static, Gzip, File>,
    artifact_path: &Path,
) -> Result<(), String> {
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
        let path = artifact_path(dump_directory, "mon_site", "20260614-223510", ".tar.gz").unwrap();

        assert_eq!(
            path,
            PathBuf::from("/tmp/pack-run/mon_site-20260614-223510.tar.gz")
        );
    }

    #[test]
    fn timestamp_label_is_sortable_and_readable() {
        let now =
            NaiveDateTime::parse_from_str("2026-06-14T22:35:10", "%Y-%m-%dT%H:%M:%S").unwrap();

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

        let extract_directory = run_directory.path().join("extract");
        std::fs::create_dir(&extract_directory).unwrap();
        let output = std::process::Command::new("tar")
            .args([
                "-xzf",
                &artifact_path.to_string_lossy(),
                "-C",
                &extract_directory.to_string_lossy(),
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "tar should extract compressed artifact. stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let dump_sql_content =
            std::fs::read_to_string(extract_directory.join("mon_site").join("dump.sql")).unwrap();

        assert_eq!(dump_sql_content, "select 1;");

        run_directory.close().unwrap();
    }
}
