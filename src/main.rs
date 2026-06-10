mod config;

use clap::Parser;

/// Rucksack 🎒 — Backup tool written in Rust
#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    /// Path to config file (default: ~/.rucksack/rucksack.yml)
    #[arg(short = 'c', long)]
    config: Option<String>,
}

fn main() {
    let cli = Cli::parse();

    let config_path = config::resolve_config_path(cli.config);
    let config = config::load_config(&config_path);

    println!("Loaded {} model(s)", config.models.len());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_help_contains_usage() {
        let err = Cli::try_parse_from(["rucksack", "--help"]).unwrap_err();
        let output = err.to_string();
        assert!(output.contains("Usage"), "Help should contain Usage");
    }

    #[test]
    fn cli_version_contains_version() {
        let err = Cli::try_parse_from(["rucksack", "--version"]).unwrap_err();
        let output = err.to_string();
        assert!(output.contains("0.1.0"), "Version should contain 0.1.0");
    }

    #[test]
    fn cli_with_config_arg_succeeds() {
        let cli = Cli::try_parse_from(["rucksack", "-c", "some/path.yml"]).unwrap();
        assert_eq!(cli.config.as_deref(), Some("some/path.yml"));
    }

    #[test]
    fn cli_without_args_succeeds() {
        let cli = Cli::try_parse_from(["rucksack"]).unwrap();
        assert!(cli.config.is_none());
    }
}
