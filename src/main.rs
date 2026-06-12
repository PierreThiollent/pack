mod archive;
mod config;
mod database;
mod logging;
mod model;
mod paths;
mod storage;

use clap::{CommandFactory, Parser, Subcommand};
use tracing::{error, info};

/// rbak 🎒 — Backup tool written in Rust 🦀
#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    /// Path to config file (default: ~/.rbak/rbak.yml)
    #[arg(short = 'c', long, global = true)]
    config: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Run backup jobs now
    Perform,
}

fn main() {
    logging::init();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Perform) => {
            let config_source = if cli.config.is_some() {
                "custom path"
            } else {
                "default path"
            };
            let config_path = config::resolve_config_path(cli.config);
            let config = config::load_config(&config_path);
            info!("[Config] Loaded config from {config_source}: {config_path}");
            if let Err(error) = model::run_all(&config) {
                error!("[Run] Failed to run backup: {error}");
                std::process::exit(1);
            }
        }
        None => {
            // No subcommand → show help
            Cli::command().print_help().unwrap();
            println!();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_help_contains_usage() {
        let err = Cli::try_parse_from(["rbak", "--help"]).unwrap_err();
        let output = err.to_string();
        assert!(output.contains("Usage"), "Help should contain Usage");
    }

    #[test]
    fn cli_version_contains_version() {
        let err = Cli::try_parse_from(["rbak", "--version"]).unwrap_err();
        let output = err.to_string();
        assert!(output.contains("0.1.0"), "Version should contain 0.1.0");
    }

    #[test]
    fn cli_with_config_arg_succeeds() {
        let cli = Cli::try_parse_from(["rbak", "-c", "some/path.yml"]).unwrap();
        assert_eq!(cli.config.as_deref(), Some("some/path.yml"));
    }

    #[test]
    fn cli_without_args_succeeds() {
        let cli = Cli::try_parse_from(["rbak"]).unwrap();
        assert!(cli.config.is_none());
    }
}
