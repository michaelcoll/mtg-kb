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
        cli::Command::Search {
            name,
            type_contains,
            text,
            color_identity,
            legal_in,
            mana_value,
            limit,
            format,
        } => commands::search::run(
            name,
            type_contains,
            text,
            color_identity,
            legal_in,
            mana_value,
            limit,
            format,
        ),
        cli::Command::Set { code, format } => commands::set::run(&code, format),
        cli::Command::Rulings { name, format } => commands::rulings::run(&name, format),
    };
    if let Err(err) = result {
        eprintln!("erreur: {err:#}");
        std::process::exit(1);
    }
}
