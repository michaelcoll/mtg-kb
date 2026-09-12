mod cli;
mod commands;
mod data_dir;
mod db;
mod model;
mod output;

use clap::Parser;

fn main() {
    let cli = cli::Cli::parse();
    let result = match cli.command {
        cli::Command::Card { name, format } => commands::card::run(&name, format),
    };
    if let Err(err) = result {
        eprintln!("erreur: {err:#}");
        std::process::exit(1);
    }
}
