use crate::config::CompressorConfig;
use std::path::{Path, PathBuf};
use tracing::info;

/// Prepare the artifact that will be uploaded by storages.
///
/// For now, compression is only wired into the pipeline as a pass-through step.
/// The real `tgz` file creation will be implemented in the next small step.
pub fn run(config: Option<&CompressorConfig>, dump_directory: &Path) -> Result<PathBuf, String> {
    match config {
        Some(CompressorConfig::Tgz) => {
            info!("[Compressor: tgz] Compression step selected");
            Ok(dump_directory.to_path_buf())
        }
        None => {
            info!("[Compressor] No compression configured; using dump directory as artifact");
            Ok(dump_directory.to_path_buf())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_without_compressor_returns_dump_directory() {
        let dump_directory = Path::new("/tmp/pack/model");

        let artifact_path = run(None, dump_directory).unwrap();

        assert_eq!(artifact_path, dump_directory);
    }

    #[test]
    fn run_with_tgz_returns_dump_directory_until_real_compression_exists() {
        let dump_directory = Path::new("/tmp/pack/model");

        let artifact_path = run(Some(&CompressorConfig::Tgz), dump_directory).unwrap();

        assert_eq!(artifact_path, dump_directory);
    }
}
