mod config;

use clap::{CommandFactory, Parser};

/// Rucksack 🎒 — Backup tool written in Rust
#[derive(Parser)]
#[command(version, about)]
struct Cli;

fn main() {
    match Cli::try_parse() {
        Ok(_) => {
            // No arguments → show help (like git, cargo, etc.)
            Cli::command().print_help().unwrap();
            println!();
        }
        Err(e) => {
            // --help, --version, or parse errors → clap handles everything
            e.exit();
        }
    }
}
