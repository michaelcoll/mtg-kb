mod cli;
mod commands;
mod data_dir;
mod db;
mod model;
mod output;
mod rules;

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
        cli::Command::Rules { action } => match action {
            cli::RulesAction::Search {
                text,
                limit,
                format,
            } => commands::rules::search(&text, limit, format),
            cli::RulesAction::Define { term, format } => commands::rules::define(&term, format),
            cli::RulesAction::Number(args) => commands::rules::dispatch_number(&args),
        },
        cli::Command::Update { target } => match target {
            None => commands::update::run_all(),
            Some(cli::UpdateTarget::Cards) => commands::update::run_cards(),
            Some(cli::UpdateTarget::Rules { url }) => commands::update_rules::run(url),
        },
    };
    if let Err(err) = result {
        eprintln!("erreur: {err:#}");
        std::process::exit(1);
    }
}
