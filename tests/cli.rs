use std::path::{Path, PathBuf};
use std::process::Command;

fn clean_test_cycler_state() {
    clean_cycler_state("my_app", "local");
}

fn clean_cycler_state(model_name: &str, storage_name: &str) {
    let home = std::env::var("HOME").expect("HOME should be set for CLI tests");
    let path = PathBuf::from(home)
        .join(".pack")
        .join("cycler")
        .join(format!("{model_name}_{storage_name}.json"));

    if path.exists() {
        std::fs::remove_file(path).expect("Failed to clean up test cycler state");
    }
}

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

    let package_version = env!("CARGO_PKG_VERSION");

    assert!(
        stdout.contains(package_version) || stderr.contains(package_version),
        "Version should contain package version. stdout:\n{stdout}\nstderr:\n{stderr}"
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
    clean_test_cycler_state();

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

    let entry_paths = list_tgz_entries(artifact_path);

    assert!(
        entry_paths.iter().any(|path| path == "my_app/archive.tar"),
        "Compressed artifact should contain the model archive.tar. Entries: {entry_paths:?}"
    );

    workspace.close().unwrap();
    clean_test_cycler_state();
}

fn list_tgz_entries(artifact_path: &Path) -> Vec<String> {
    let output = Command::new("tar")
        .args(["-tzf", &artifact_path.to_string_lossy()])
        .output()
        .expect("Failed to list compressed tar artifact");

    assert!(
        output.status.success(),
        "tar should list compressed artifact. stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_owned)
        .collect()
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
fn final_cli_errors_are_logged_with_tracing() {
    let workspace = tempfile::tempdir().unwrap();
    let missing_include = workspace.path().join("missing.txt");
    let storage_directory = workspace.path().join("backups");
    let config_path = workspace.path().join("pack.yml");

    let config_content = format!(
        r#"
models:
  my_app:
    databases: {{}}
    archive:
      includes:
        - {missing_include}
    storages:
      local:
        type: local
        path: {storage_directory}
"#,
        missing_include = missing_include.display(),
        storage_directory = storage_directory.display()
    );
    std::fs::write(&config_path, config_content).unwrap();

    let output = run_pack(&["perform", "-c", &config_path.to_string_lossy()]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success(), "pack should exit with error");
    assert!(
        stderr.contains("ERROR")
            && stderr.contains("[Run]")
            && stderr.contains("Failed to run backup"),
        "Final CLI error should use tracing with level and tag. stderr:\n{stderr}"
    );
}

#[test]
fn load_config_with_unsafe_named_key_errors() {
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
    storages:
      ../escaped:
        type: local
        path: {storage_directory}
"#,
        workdir = workspace.path().display(),
        included_file = included_file.display(),
        storage_directory = storage_directory.display()
    );
    std::fs::write(&config_path, config_content).unwrap();

    let output = run_pack(&["perform", "-c", &config_path.to_string_lossy()]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "pack should exit with error on unsafe YAML keys"
    );
    assert!(
        stderr.contains("Invalid storage name"),
        "Should show validation error. stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        stderr
    );
}

#[test]
fn local_storage_retention_keeps_only_latest_artifact() {
    let model_name = "retention_app";
    let storage_name = "local";
    clean_cycler_state(model_name, storage_name);

    let workspace = tempfile::tempdir().unwrap();
    let included_file = workspace.path().join("included.txt");
    let storage_directory = workspace.path().join("backups");
    let config_path = workspace.path().join("pack.yml");

    std::fs::write(&included_file, "first content").unwrap();

    let config_content = format!(
        r#"
workdir: {workdir}
models:
  {model_name}:
    databases: {{}}
    archive:
      includes:
        - {included_file}
    compress_with:
      type: tgz
    storages:
      {storage_name}:
        type: local
        path: {storage_directory}
        keep: 1
"#,
        workdir = workspace.path().display(),
        included_file = included_file.display(),
        storage_directory = storage_directory.display()
    );
    std::fs::write(&config_path, config_content).unwrap();

    let first_output = run_pack(&["perform", "-c", &config_path.to_string_lossy()]);
    assert!(
        first_output.status.success(),
        "first run should exit successfully. stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&first_output.stdout),
        String::from_utf8_lossy(&first_output.stderr)
    );

    std::thread::sleep(std::time::Duration::from_secs(1));
    std::fs::write(&included_file, "second content").unwrap();

    let second_output = run_pack(&["perform", "-c", &config_path.to_string_lossy()]);
    assert!(
        second_output.status.success(),
        "second run should exit successfully. stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&second_output.stdout),
        String::from_utf8_lossy(&second_output.stderr)
    );

    let artifacts = tar_gz_files_in(&storage_directory);
    assert_eq!(
        artifacts.len(),
        1,
        "Expected retention to keep one artifact"
    );

    clean_cycler_state(model_name, storage_name);
}

#[test]
fn load_config_with_missing_environment_variable_errors() {
    let workspace = tempfile::tempdir().unwrap();
    let config_path = workspace.path().join("pack.yml");

    std::fs::write(
        &config_path,
        r#"
models:
  my_app:
    databases: {}
    archive:
      includes:
        - $PACK_TEST_VARIABLE_THAT_SHOULD_NOT_EXIST
    storages: {}
"#,
    )
    .unwrap();

    let output = run_pack(&["perform", "-c", &config_path.to_string_lossy()]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "pack should exit with error when an environment variable is missing"
    );
    assert!(
        stderr.contains("Failed to expand environment variables"),
        "Should show environment expansion error. stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        stderr
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
