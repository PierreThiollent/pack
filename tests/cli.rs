use flate2::read::GzDecoder;
use std::fs::File;
use std::path::Path;
use std::process::Command;

/// Helper: run `cargo run -- <args>` and return the output
fn run_pack(args: &[&str]) -> std::process::Output {
    Command::new("cargo")
        .args(["run", "--quiet", "--"])
        .args(args)
        .output()
        .expect("Failed to run pack")
}

#[test]
fn help_displays_usage() {
    let output = run_pack(&["--help"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stdout.contains("Usage") || stderr.contains("Usage"),
        "Help should contain 'Usage'. stdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[test]
fn version_displays_version() {
    let output = run_pack(&["--version"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stdout.contains("0.1.0") || stderr.contains("0.1.0"),
        "Version should contain 0.1.0. stdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[test]
fn load_valid_config_file() {
    // Create a temporary config file (empty — just test that the CLI works)
    let config_content = "models: {}
";

    let dir = std::env::temp_dir().join("pack-test");
    std::fs::create_dir_all(&dir).unwrap();
    let config_path = dir.join("pack.yml");
    std::fs::write(&config_path, config_content).unwrap();

    let output = run_pack(&["perform", "-c", &config_path.to_string_lossy()]);

    assert!(output.status.success(), "pack should exit successfully");

    // Cleanup
    if let Err(error) = std::fs::remove_dir_all(&dir) {
        panic!("Failed to clean up test directory {dir:?}: {error}");
    }
}

#[test]
fn perform_archives_compresses_and_stores_local_artifact() {
    let workspace = tempfile::tempdir().unwrap();
    let included_file = workspace.path().join("included.txt");
    let storage_directory = workspace.path().join("backups");
    let config_path = workspace.path().join("pack.yml");

    std::fs::write(&included_file, "included content").unwrap();

    let config_content = format!(
        r#"
workdir: {workdir}
models:
  my_app:
    databases: {{}}
    archive:
      includes:
        - {included_file}
    compress_with:
      type: tgz
    storages:
      local:
        type: local
        path: {storage_directory}
"#,
        workdir = workspace.path().display(),
        included_file = included_file.display(),
        storage_directory = storage_directory.display()
    );
    std::fs::write(&config_path, config_content).unwrap();

    let output = run_pack(&["perform", "-c", &config_path.to_string_lossy()]);

    assert!(
        output.status.success(),
        "pack should exit successfully. stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let artifacts = tar_gz_files_in(&storage_directory);
    assert_eq!(artifacts.len(), 1, "Expected one local artifact");

    let artifact_path = &artifacts[0];
    assert!(
        artifact_path
            .file_name()
            .and_then(|file_name| file_name.to_str())
            .is_some_and(
                |file_name| file_name.starts_with("my_app-") && file_name.ends_with(".tar.gz")
            ),
        "Unexpected artifact name: {artifact_path:?}"
    );

    let artifact_file = File::open(artifact_path).unwrap();
    let decoder = GzDecoder::new(artifact_file);
    let mut archive = tar::Archive::new(decoder);
    let entry_paths: Vec<_> = archive
        .entries()
        .unwrap()
        .map(|entry| entry.unwrap().path().unwrap().into_owned())
        .collect();

    assert!(
        entry_paths
            .iter()
            .any(|path| path == Path::new("my_app/archive.tar")),
        "Compressed artifact should contain the model archive.tar. Entries: {entry_paths:?}"
    );

    workspace.close().unwrap();
}

fn tar_gz_files_in(directory: &Path) -> Vec<std::path::PathBuf> {
    let mut files: Vec<_> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .and_then(|file_name| file_name.to_str())
                .is_some_and(|file_name| file_name.ends_with(".tar.gz"))
        })
        .collect();
    files.sort();
    files
}

#[test]
fn load_missing_config_file_errors() {
    let output = run_pack(&["perform", "-c", "/tmp/pack-nonexistent.yml"]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success(), "pack should exit with error");
    assert!(
        stderr.contains("Failed to read config file"),
        "Should show file error. stdout:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn load_invalid_yaml_errors() {
    let dir = std::env::temp_dir().join("pack-test-invalid");
    std::fs::create_dir_all(&dir).unwrap();
    let config_path = dir.join("pack.yml");
    std::fs::write(&config_path, "invalid: [yaml: broken").unwrap();

    let output = run_pack(&["perform", "-c", &config_path.to_string_lossy()]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "pack should exit with error on invalid YAML"
    );
    assert!(
        stderr.contains("Failed to parse config file"),
        "Should show parse error. stdout:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );

    if let Err(error) = std::fs::remove_dir_all(&dir) {
        panic!("Failed to clean up test directory {dir:?}: {error}");
    }
}
