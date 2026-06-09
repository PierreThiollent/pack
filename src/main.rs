mod config;

use clap::Parser;

/// Rucksack 🎒 — Backup tool written in Rust
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// Path to config file (default: ~/.rucksack/rucksack.yml)
    #[arg(short = 'c', long)]
    config: Option<String>,
}

fn main() {
    let cli = Cli::parse();

    // Determine config path: CLI arg or default ~/.rucksack/rucksack.yml
    let config_path = cli.config.unwrap_or_else(|| {
        let home = std::env::var("HOME").expect("Could not find HOME environment variable");
        format!("{home}/.rucksack/rucksack.yml")
    });

    let yaml_content = std::fs::read_to_string(&config_path).unwrap_or_else(|e| {
        eprintln!("Error reading config file {config_path}: {e}");
        std::process::exit(1);
    });

    let config: config::Config = serde_yaml::from_str(&yaml_content).unwrap_or_else(|e| {
        eprintln!("Error parsing config file {config_path}: {e}");
        std::process::exit(1);
    });

    println!("Loaded {} model(s)", config.models.len());
}
