use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::analyze::metrics::Thresholds;
use crate::db::overrides::CorrectionField;
use crate::output::Format;

#[derive(Parser)]
#[command(name = "kb", about = "Base de connaissances Magic: The Gathering")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[allow(clippy::large_enum_variant)]
#[derive(Subcommand)]
pub enum Command {
    /// Analyse une Decklist Commander (fichier, ou "-" pour stdin)
    Analyze {
        /// Chemin du fichier, ou "-" pour lire depuis l'entrée standard
        source: String,
        #[command(flatten)]
        thresholds: Thresholds,
        /// Désactive les appels aux Sources externes (EDHREC, Recommander) :
        /// utile pour les tests ou un usage hors ligne
        #[arg(long, default_value_t = false)]
        offline: bool,
    },
    /// Génère le Rapport d'analyse HTML à partir du JSON de `kb analyze`
    /// enrichi (verdict, Suggestions retenues)
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
    Search(SearchArgs),
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
    /// Corrections manuelles des Rôles, Thèmes et légalité Commander d'une
    /// Carte (data/overrides.sqlite), appliquées partout où la Carte est lue
    Override {
        #[command(subcommand)]
        action: OverrideAction,
    },
}

#[derive(Subcommand)]
pub enum OverrideAction {
    /// Remplace les Rôles détectés d'une Carte
    Role(LabelsArgs),
    /// Remplace les Thèmes détectés d'une Carte
    Theme(LabelsArgs),
    /// Fixe la légalité Commander d'une Carte
    Legality {
        /// Nom de la Carte (nom oracle ou de sa Face principale)
        card: String,
        #[arg(value_enum)]
        value: Legality,
        /// Motif de la Correction (obligatoire)
        #[arg(long)]
        reason: String,
    },
    /// Liste les Corrections : Carte, champ, valeur, motif, date
    List {
        #[arg(long, value_enum, default_value_t = Format::Json)]
        format: Format,
    },
    /// Retire une Correction : la détection de `kb` s'applique de nouveau
    Remove {
        #[arg(value_enum)]
        field: CorrectionField,
        /// Nom de la Carte (nom oracle ou de sa Face principale)
        card: String,
    },
}

#[derive(Debug, Clone, PartialEq, Args)]
pub struct SearchArgs {
    /// Nom (recherche partielle)
    #[arg(long)]
    pub name: Option<String>,
    /// Sous-chaîne de la ligne de type (ex. "Creature", "Legendary")
    #[arg(long = "type")]
    pub type_contains: Option<String>,
    /// Sous-chaîne des sous-types (ex. "Elf", "Equipment")
    #[arg(long = "subtype")]
    pub subtype_contains: Option<String>,
    /// Sous-chaîne du texte oracle (répétable, combiné en ET)
    #[arg(long)]
    pub text: Vec<String>,
    /// Identité de couleur autorisée, ex. "WU" : la Carte doit y être incluse
    #[arg(long = "color-identity")]
    pub color_identity: Option<String>,
    /// Format de légalité requis (ex. commander, standard)
    #[arg(long = "legal-in")]
    pub legal_in: Option<String>,
    /// Mana value exacte
    #[arg(long = "mana-value")]
    pub mana_value: Option<f64>,
    /// Mana value minimale (borne incluse)
    #[arg(long = "mana-value-min")]
    pub mana_value_min: Option<f64>,
    /// Mana value maximale (borne incluse)
    #[arg(long = "mana-value-max")]
    pub mana_value_max: Option<f64>,
    /// Rôle détecté requis (même détection que `kb analyze`), répétable,
    /// combiné en ET
    #[arg(long = "role")]
    pub role: Vec<String>,
    /// Thème détecté requis (même détection que `kb analyze`), répétable,
    /// combiné en ET
    #[arg(long = "theme")]
    pub theme: Vec<String>,
    /// Exclut les Cartes présentes dans cette Decklist (même parsing que
    /// `kb analyze`) ; fichier, ou "-" pour lire depuis l'entrée standard
    #[arg(long = "exclude-deck")]
    pub exclude_deck: Option<String>,
    #[arg(long, default_value_t = 50)]
    pub limit: usize,
    #[arg(long, value_enum, default_value_t = Format::Json)]
    pub format: Format,
}

#[derive(Args)]
pub struct LabelsArgs {
    /// Nom de la Carte (nom oracle ou de sa Face principale)
    pub card: String,
    /// Liste finale, séparée par des virgules ; remplace entièrement la
    /// détection. Toute valeur est acceptée, mise en snake_case minuscule
    #[arg(required_unless_present = "none", conflicts_with = "none")]
    pub values: Option<String>,
    /// Fixe une liste vide
    #[arg(long)]
    pub none: bool,
    /// Motif de la Correction (obligatoire)
    #[arg(long)]
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Legality {
    Legal,
    Banned,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("kb").chain(args.iter().copied()))
    }

    fn analyze_thresholds(args: &[&str]) -> Thresholds {
        match parse(args).unwrap().command {
            Command::Analyze { thresholds, .. } => thresholds,
            _ => panic!("kb analyze attendu"),
        }
    }

    #[test]
    fn analyze_threshold_defaults_are_the_domain_defaults() {
        assert_eq!(
            analyze_thresholds(&["analyze", "deck.txt"]),
            Thresholds::default()
        );
    }

    #[test]
    fn analyze_threshold_options_override_the_defaults() {
        assert_eq!(
            analyze_thresholds(&[
                "analyze",
                "deck.txt",
                "--min-lands",
                "30",
                "--max-average-mana-value",
                "2.5",
            ]),
            Thresholds {
                min_lands: 30,
                max_average_mana_value: 2.5,
                ..Thresholds::default()
            }
        );
    }

    #[test]
    fn search_args_default_limit_and_format() {
        let Command::Search(args) = parse(&["search"]).unwrap().command else {
            panic!("kb search attendu");
        };
        assert_eq!(args.limit, 50);
        assert_eq!(args.format, Format::Json);
        assert!(args.role.is_empty() && args.exclude_deck.is_none());
    }

    #[test]
    fn override_role_requires_a_reason() {
        assert!(parse(&["override", "role", "Bite Down", "removal_cible"]).is_err());
        assert!(parse(&["override", "legality", "Bite Down", "banned"]).is_err());
        assert!(
            parse(&[
                "override",
                "role",
                "Bite Down",
                "removal_cible",
                "--reason",
                "bite"
            ])
            .is_ok()
        );
    }

    #[test]
    fn override_role_takes_values_or_none_but_not_both() {
        assert!(parse(&["override", "theme", "Bite Down", "--reason", "x"]).is_err());
        assert!(parse(&["override", "theme", "Bite Down", "--none", "--reason", "x"]).is_ok());
        assert!(
            parse(&[
                "override",
                "theme",
                "Bite Down",
                "tokens",
                "--none",
                "--reason",
                "x"
            ])
            .is_err()
        );
    }

    #[test]
    fn override_legality_accepts_only_legal_or_banned() {
        assert!(
            parse(&[
                "override",
                "legality",
                "Sol Ring",
                "restricted",
                "--reason",
                "x"
            ])
            .is_err()
        );
    }

    #[test]
    fn override_list_and_remove_take_no_reason() {
        assert!(parse(&["override", "list"]).is_ok());
        assert!(parse(&["override", "remove", "legality", "Sol Ring"]).is_ok());
    }
}
