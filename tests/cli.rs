use std::process::Command;

/// Helper: run `cargo run -- <args>` and return the output
fn run_rucksack(args: &[&str]) -> std::process::Output {
    Command::new("cargo")
        .args(["run", "--quiet", "--"])
        .args(args)
        .output()
        .expect("Failed to run rucksack")
}

#[test]
fn help_displays_usage() {
    let output = run_rucksack(&["--help"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stdout.contains("Usage") || stderr.contains("Usage"),
        "Help should contain 'Usage'. stdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[test]
fn version_displays_version() {
    let output = run_rucksack(&["--version"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stdout.contains("0.1.0") || stderr.contains("0.1.0"),
        "Version should contain 0.1.0. stdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[test]
fn load_valid_config_file() {
    // Create a temporary config file
    let config_content = r#"
models:
  test_app:
    databases:
      test_db:
        type: mysql
        host: localhost
        database: testdb
"#;

    let dir = std::env::temp_dir().join("rucksack-test");
    std::fs::create_dir_all(&dir).unwrap();
    let config_path = dir.join("rucksack.yml");
    std::fs::write(&config_path, config_content).unwrap();

    let output = run_rucksack(&["perform", "-c", &config_path.to_string_lossy()]);

    assert!(output.status.success(), "rucksack should exit successfully");

    // Cleanup
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn load_missing_config_file_errors() {
    let output = run_rucksack(&["perform", "-c", "/tmp/rucksack-nonexistent.yml"]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success(), "rucksack should exit with error");
    assert!(
        stderr.contains("Error reading config file"),
        "Should show file error. stdout:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn load_invalid_yaml_errors() {
    let dir = std::env::temp_dir().join("rucksack-test-invalid");
    std::fs::create_dir_all(&dir).unwrap();
    let config_path = dir.join("rucksack.yml");
    std::fs::write(&config_path, "invalid: [yaml: broken").unwrap();

    let output = run_rucksack(&["perform", "-c", &config_path.to_string_lossy()]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "rucksack should exit with error on invalid YAML"
    );
    assert!(
        stderr.contains("Error parsing config file"),
        "Should show parse error. stdout:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );

    let _ = std::fs::remove_dir_all(&dir);
}
