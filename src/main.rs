use clap::{CommandFactory, Parser};

/// Rucksack 🎒 — Backup tool written in Rust
#[derive(Parser)]
#[command(version, about)]
struct Cli;

fn main() {
    match Cli::try_parse() {
        Ok(_) => {
            // Aucun argument : on affiche l'aide (comme git, cargo, etc.)
            Cli::command().print_help().unwrap();
            println!();
        }
        Err(e) => {
            // --help, --version, ou erreur de syntaxe → clap gère tout
            e.exit();
        }
    }
}

