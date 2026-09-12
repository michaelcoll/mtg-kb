use clap::{Parser, Subcommand};

use crate::output::Format;

#[derive(Parser)]
#[command(name = "kb", about = "Base de connaissances Magic: The Gathering")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Affiche une Carte par son nom oracle exact
    Card {
        name: String,
        #[arg(long, value_enum, default_value_t = Format::Json)]
        format: Format,
    },
}
