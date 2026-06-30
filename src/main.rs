mod archive;
mod compressor;
mod config;
mod cycler;
mod daemon;
mod database;
mod logging;
mod model;
mod paths;
mod scheduler;
mod storage;

use clap::{CommandFactory, Parser, Subcommand};
use daemon::DaemonProcess;
use logging::{LogDestination, LogTag, tag};
use tracing::info;

/// pack 🎒 — Backup tool written in Rust 🦀
#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    /// Path to config file (default: ~/.pack/pack.yml)
    #[arg(short = 'c', long, global = true)]
    config: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Run backup jobs now
    Perform,
    /// Run the scheduler in the foreground
    Run,
    /// Start the scheduler in the background
    Start,
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Some(Commands::Perform) => perform_backups_now(cli.config),
        Some(Commands::Run) => run_scheduler_foreground(cli.config),
        Some(Commands::Start) => start_scheduler_daemon(cli.config),
        None => print_help(),
    };

    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

// `perform` runs backups immediately and logs only to the console.
fn perform_backups_now(config_arg: Option<String>) -> Result<(), String> {
    logging::init(LogDestination::ConsoleOnly)?;
    let config = load_cli_config(config_arg);

    model::run_all(&config).map_err(|error| format!("Failed to run backup: {error}"))
}

// `run` keeps the scheduler in the foreground and logs to console + file.
fn run_scheduler_foreground(config_arg: Option<String>) -> Result<(), String> {
    logging::init(LogDestination::ConsoleAndFile(paths::pack_log_file_path()))?;
    let config = load_cli_config(config_arg);

    run_scheduler(config)
}

fn print_help() -> Result<(), String> {
    logging::init(LogDestination::ConsoleOnly)?;
    Cli::command()
        .print_help()
        .map_err(|error| format!("Failed to print help: {error}"))?;
    println!();
    Ok(())
}

// `start` daemonizes first, then the child process logs only to the file.
fn start_scheduler_daemon(config_arg: Option<String>) -> Result<(), String> {
    let log_file_path = paths::pack_log_file_path();
    let pid_file_path = paths::pack_pid_file_path();

    match daemon::start(&pid_file_path)? {
        DaemonProcess::Parent => {
            println!("Pack daemon started.");
            println!("Log file: {}", log_file_path.display());
            println!("PID file: {}", pid_file_path.display());
            Ok(())
        }
        DaemonProcess::Child => {
            logging::init(LogDestination::FileOnly(log_file_path))?;
            let config = load_cli_config(config_arg);
            info!(pack_tag = %tag(LogTag::Run), "Started in background");
            run_scheduler(config)
        }
    }
}

// Create Tokio after daemonization so the child does not inherit a pre-fork runtime.
fn run_scheduler(config: config::Config) -> Result<(), String> {
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|error| format!("Failed to create Tokio runtime: {error}"))?;

    runtime
        .block_on(scheduler::run_foreground(config))
        .map_err(|error| format!("Scheduler failed: {error}"))
}

fn load_cli_config(config_arg: Option<String>) -> config::Config {
    let config_source = if config_arg.is_some() {
        "custom path"
    } else {
        "default path"
    };
    let config_path = config::resolve_config_path(config_arg);
    let config = config::load_config(&config_path);
    info!(
        pack_tag = %tag(LogTag::Config),
        "Loaded config from {config_source}: {config_path}"
    );

    config
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_help_contains_usage() {
        let err = Cli::try_parse_from(["pack", "--help"]).unwrap_err();
        let output = err.to_string();
        assert!(output.contains("Usage"), "Help should contain Usage");
    }

    #[test]
    fn cli_version_contains_version() {
        let err = Cli::try_parse_from(["pack", "--version"]).unwrap_err();
        let output = err.to_string();
        assert!(output.contains("0.1.0"), "Version should contain 0.1.0");
    }

    #[test]
    fn cli_with_config_arg_succeeds() {
        let cli = Cli::try_parse_from(["pack", "-c", "some/path.yml"]).unwrap();
        assert_eq!(cli.config.as_deref(), Some("some/path.yml"));
    }

    #[test]
    fn cli_without_args_succeeds() {
        let cli = Cli::try_parse_from(["pack"]).unwrap();
        assert!(cli.config.is_none());
    }

    #[test]
    fn cli_start_subcommand_succeeds() {
        let cli = Cli::try_parse_from(["pack", "start"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Start)));
    }
}
