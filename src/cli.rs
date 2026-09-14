use clap::{Parser, Subcommand};

use crate::output::Format;

#[derive(Parser)]
#[command(name = "kb", about = "Base de connaissances Magic: The Gathering")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

// Le CLI n'est parsé qu'une seule fois au démarrage : la différence de
// taille entre variantes (ex. `Search`, qui accumule de nombreux filtres
// optionnels) n'a pas d'impact de performance mesurable.
#[allow(clippy::large_enum_variant)]
#[derive(Subcommand)]
pub enum Command {
    /// Analyse une Decklist Commander (fichier, ou "-" pour stdin)
    Analyze {
        /// Chemin du fichier, ou "-" pour lire depuis l'entrée standard
        source: String,
        /// Nombre minimal de terrains attendu
        #[arg(long, default_value_t = 35)]
        min_lands: u32,
        /// Nombre minimal de Cartes de Rôle "ramp" attendu
        #[arg(long, default_value_t = 10)]
        min_ramp: u32,
        /// Nombre minimal de Cartes de Rôle "pioche" attendu
        #[arg(long, default_value_t = 8)]
        min_draw: u32,
        /// Nombre minimal de Cartes de Rôle "removal_cible" attendu
        #[arg(long, default_value_t = 8)]
        min_removal: u32,
        /// Nombre minimal de Cartes de Rôle "wipe" attendu
        #[arg(long, default_value_t = 2)]
        min_wipe: u32,
        /// Mana value moyenne (hors terrains) au-delà de laquelle la courbe
        /// est jugée trop chère
        #[arg(long, default_value_t = 3.5)]
        max_average_mana_value: f64,
        /// Nombre de Cartes à mana value ≥ 6 au-delà duquel la courbe est
        /// jugée déséquilibrée vers le haut
        #[arg(long, default_value_t = 8)]
        max_high_cost_cards: u32,
    },
    /// Génère le Rapport d'analyse HTML à partir du JSON de `kb analyze`
    /// enrichi par Claude (verdict, Suggestions retenues)
    Report {
        /// Chemin du fichier JSON enrichi
        json_path: String,
    },
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
        /// Sous-chaîne de la ligne de type (ex. "Creature", "Legendary")
        #[arg(long = "type")]
        type_contains: Option<String>,
        /// Sous-chaîne des sous-types (ex. "Elf", "Equipment")
        #[arg(long = "subtype")]
        subtype_contains: Option<String>,
        /// Sous-chaîne du texte oracle (répétable, combiné en ET)
        #[arg(long)]
        text: Vec<String>,
        /// Identité de couleur autorisée, ex. "WU" : la Carte doit y être incluse
        #[arg(long = "color-identity")]
        color_identity: Option<String>,
        /// Format de légalité requis (ex. commander, standard)
        #[arg(long = "legal-in")]
        legal_in: Option<String>,
        /// Mana value exacte
        #[arg(long = "mana-value")]
        mana_value: Option<f64>,
        /// Mana value minimale (borne incluse)
        #[arg(long = "mana-value-min")]
        mana_value_min: Option<f64>,
        /// Mana value maximale (borne incluse)
        #[arg(long = "mana-value-max")]
        mana_value_max: Option<f64>,
        /// Rôle détecté requis (même détection que `kb analyze`), répétable,
        /// combiné en ET
        #[arg(long = "role")]
        role: Vec<String>,
        /// Thème détecté requis (même détection que `kb analyze`), répétable,
        /// combiné en ET
        #[arg(long = "theme")]
        theme: Vec<String>,
        /// Exclut les Cartes présentes dans cette Decklist (même parsing que
        /// `kb analyze`) ; fichier, ou "-" pour lire depuis l'entrée standard
        #[arg(long = "exclude-deck")]
        exclude_deck: Option<String>,
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
