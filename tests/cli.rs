use std::process::Command;

/// Helper: run `cargo run -- <args>` and return the output
fn run_rbak(args: &[&str]) -> std::process::Output {
    Command::new("cargo")
        .args(["run", "--quiet", "--"])
        .args(args)
        .output()
        .expect("Failed to run rbak")
}

#[test]
fn help_displays_usage() {
    let output = run_rbak(&["--help"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stdout.contains("Usage") || stderr.contains("Usage"),
        "Help should contain 'Usage'. stdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[test]
fn version_displays_version() {
    let output = run_rbak(&["--version"]);
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

    let dir = std::env::temp_dir().join("rbak-test");
    std::fs::create_dir_all(&dir).unwrap();
    let config_path = dir.join("rbak.yml");
    std::fs::write(&config_path, config_content).unwrap();

    let output = run_rbak(&["perform", "-c", &config_path.to_string_lossy()]);

    assert!(output.status.success(), "rbak should exit successfully");

    // Cleanup
    if let Err(error) = std::fs::remove_dir_all(&dir) {
        panic!("Failed to clean up test directory {dir:?}: {error}");
    }
}

#[test]
fn load_missing_config_file_errors() {
    let output = run_rbak(&["perform", "-c", "/tmp/rbak-nonexistent.yml"]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success(), "rbak should exit with error");
    assert!(
        stderr.contains("Failed to read config file"),
        "Should show file error. stdout:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn load_invalid_yaml_errors() {
    let dir = std::env::temp_dir().join("rbak-test-invalid");
    std::fs::create_dir_all(&dir).unwrap();
    let config_path = dir.join("rbak.yml");
    std::fs::write(&config_path, "invalid: [yaml: broken").unwrap();

    let output = run_rbak(&["perform", "-c", &config_path.to_string_lossy()]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "rbak should exit with error on invalid YAML"
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
