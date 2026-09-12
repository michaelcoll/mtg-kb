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
    /// Recherche des Cartes par filtres combinés
    Search {
        /// Nom (recherche partielle)
        #[arg(long)]
        name: Option<String>,
        /// Sous-chaîne de la ligne de type (ex. "Creature", "Elf")
        #[arg(long = "type")]
        type_contains: Option<String>,
        /// Sous-chaîne du texte oracle
        #[arg(long)]
        text: Option<String>,
        /// Identité de couleur autorisée, ex. "WU" : la Carte doit y être incluse
        #[arg(long = "color-identity")]
        color_identity: Option<String>,
        /// Format de légalité requis (ex. commander, standard)
        #[arg(long = "legal-in")]
        legal_in: Option<String>,
        /// Mana value exacte
        #[arg(long = "mana-value")]
        mana_value: Option<f64>,
        #[arg(long, default_value_t = 50)]
        limit: usize,
        #[arg(long, value_enum, default_value_t = Format::Json)]
        format: Format,
    },
    /// Affiche les informations d'un Set par son code
    Set {
        code: String,
        #[arg(long, value_enum, default_value_t = Format::Json)]
        format: Format,
    },
    /// Affiche les Rulings datés d'une Carte
    Rulings {
        name: String,
        #[arg(long, value_enum, default_value_t = Format::Json)]
        format: Format,
    },
    /// Interroge la Base règles (Comprehensive Rules)
    Rules {
        #[command(subcommand)]
        action: RulesAction,
    },
    /// Met à jour les bases locales (cartes et/ou règles)
    Update {
        #[command(subcommand)]
        target: Option<UpdateTarget>,
    },
}

#[derive(Subcommand)]
pub enum UpdateTarget {
    /// Ne met à jour que la Base cartes (MTGJSON AllPrintings.sqlite)
    Cards,
    /// Ne met à jour que la Base règles (Comprehensive Rules)
    Rules {
        /// URL directe du fichier .txt (sinon, découverte automatique)
        #[arg(long)]
        url: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum RulesAction {
    /// Recherche plein texte dans les Règles
    Search {
        text: String,
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long, value_enum, default_value_t = Format::Json)]
        format: Format,
    },
    /// Définition d'un terme du Glossaire
    Define {
        term: String,
        #[arg(long, value_enum, default_value_t = Format::Json)]
        format: Format,
    },
    /// `kb rules <numéro>` : affiche une Règle (ou Section) et ses
    /// sous-Règles. Capturé ici car un numéro de Règle n'est ni "search" ni
    /// "define".
    #[command(external_subcommand)]
    Number(Vec<String>),
}
