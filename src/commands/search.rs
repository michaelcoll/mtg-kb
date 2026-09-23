use std::collections::HashSet;

use anyhow::Result;

use crate::analyze::metrics::detect_roles;
use crate::analyze::themes::detect_themes;
use crate::cli::SearchArgs;
use crate::db::cards::{CardsDb, SearchFilters};
use crate::decklist::{self, parser};
use crate::model::{Card, ColorIdentity};
use crate::output::{Format, print_json};

pub fn run(args: SearchArgs) -> Result<()> {
    let db = super::open_cards_db()?;
    let exclude_names = args
        .exclude_deck
        .as_deref()
        .map(names_in_decklist)
        .transpose()?;

    let results = search_cards(&db, &args, exclude_names.as_ref())?;

    match args.format {
        Format::Json => print_json(&results)?,
        Format::Table => print_table(&results),
    }
    Ok(())
}

/// Les filtres Rôle/Thème et l'exclusion par Decklist sont appliqués après
/// la requête SQL, donc `limit` ne s'applique qu'à la fin. `exclude_names`
/// remplace la lecture de `args.exclude_deck`, déjà faite par l'appelant.
fn search_cards(
    db: &CardsDb,
    args: &SearchArgs,
    exclude_names: Option<&HashSet<String>>,
) -> Result<Vec<Card>> {
    let needs_post_filter =
        !args.role.is_empty() || !args.theme.is_empty() || exclude_names.is_some();

    let filters = SearchFilters {
        name: args.name.clone(),
        type_contains: args.type_contains.clone(),
        subtype_contains: args.subtype_contains.clone(),
        oracle_text_contains: args.text.clone(),
        color_identity_subset_of: args
            .color_identity
            .as_deref()
            .map(ColorIdentity::from_letters),
        legal_in_format: args.legal_in.clone(),
        mana_value: args.mana_value,
        mana_value_min: args.mana_value_min,
        mana_value_max: args.mana_value_max,
        limit: if needs_post_filter { 0 } else { args.limit },
    };
    let mut results = db.search(&filters)?;

    if !args.role.is_empty() {
        results.retain(|card| card_matches_all(&detect_roles(card), &args.role));
    }
    if !args.theme.is_empty() {
        results.retain(|card| card_matches_all(&detect_themes(card), &args.theme));
    }
    if let Some(names) = exclude_names {
        results.retain(|card| !names.contains(&card.name));
    }
    if needs_post_filter && args.limit > 0 {
        results.truncate(args.limit);
    }
    Ok(results)
}

fn card_matches_all(detected: &[String], wanted: &[String]) -> bool {
    wanted
        .iter()
        .all(|w| detected.iter().any(|d| d.eq_ignore_ascii_case(w)))
}

/// `path` peut être "-" pour lire depuis l'entrée standard.
fn names_in_decklist(path: &str) -> Result<HashSet<String>> {
    Ok(names_in_decklist_text(&decklist::read_source(path)?))
}

fn names_in_decklist_text(input: &str) -> HashSet<String> {
    let decklist = parser::parse(input);
    decklist
        .commander
        .into_iter()
        .chain(decklist.deck)
        .map(|line| line.name)
        .collect()
}

fn print_table(cards: &[Card]) {
    for card in cards {
        println!(
            "{:<30} {:<6} {}",
            card.name,
            card.front.mana_cost.clone().unwrap_or_default(),
            card.front.type_line.clone().unwrap_or_default()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::fixture::{CardsFixture, FixtureCard};
    use crate::db::overrides::Corrections;
    use crate::model::CardCorrections;

    fn fixture_db() -> (tempfile::TempDir, CardsDb) {
        CardsFixture::new()
            .cards([
                FixtureCard::new("doom-blade", "Doom Blade")
                    .mana("{1}{B}", 2.0)
                    .types("Instant")
                    .text("Destroy target creature.")
                    .identity("B"),
                FixtureCard::new("beast-within", "Beast Within")
                    .mana("{2}{G}", 3.0)
                    .types("Sorcery")
                    .text(
                        "Destroy target permanent. Its controller creates a 3/3 green Beast \
                         creature token.",
                    )
                    .identity("G"),
                FixtureCard::new("terminate", "Terminate")
                    .mana("{B}{R}", 2.0)
                    .types("Instant")
                    .text("Destroy target creature or planeswalker.")
                    .identity("B, R"),
                FixtureCard::new("damnation", "Damnation")
                    .mana("{2}{B}{B}", 4.0)
                    .types("Sorcery")
                    .text("Destroy all creatures.")
                    .identity("B"),
                FixtureCard::new("banned-removal", "Hypothetical Banned Removal")
                    .mana("{B}", 1.0)
                    .types("Instant")
                    .text("Destroy target creature.")
                    .identity("B")
                    .banned(),
                FixtureCard::new("vanilla-bear", "Vanilla Bear")
                    .mana("{1}{G}", 2.0)
                    .types("Creature")
                    .subtypes("Bear")
                    .identity("G"),
                FixtureCard::new("rampant", "Rampant Growth")
                    .mana("{1}{G}", 2.0)
                    .types("Sorcery")
                    .text(
                        "Search your library for a basic land card and put it onto the \
                         battlefield tapped.",
                    )
                    .identity("G"),
                FixtureCard::new("goblin-king", "Goblin King")
                    .mana("{1}{R}{R}", 3.0)
                    .types("Creature")
                    .subtypes("Goblin")
                    .text("Other Goblins get +1/+1.")
                    .identity("R"),
            ])
            .build()
    }

    /// `options` : options de `kb search`, parsées comme par la CLI.
    fn search(
        db: &CardsDb,
        options: &[&str],
        exclude_names: Option<&HashSet<String>>,
    ) -> Vec<Card> {
        use clap::Parser as _;
        let cli = crate::cli::Cli::try_parse_from(["kb", "search"].iter().chain(options)).unwrap();
        let crate::cli::Command::Search(args) = cli.command else {
            panic!("kb search attendu");
        };
        search_cards(db, &args, exclude_names).unwrap()
    }

    #[test]
    fn filters_by_role() {
        let (_dir, db) = fixture_db();
        let results = search(&db, &["--role", "ramp"], None);
        let names: Vec<_> = results.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["Rampant Growth"]);
    }

    #[test]
    fn filters_by_theme() {
        let (_dir, db) = fixture_db();
        let results = search(&db, &["--theme", "tribal:Goblin"], None);
        let names: Vec<_> = results.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["Goblin King"]);
    }

    #[test]
    fn excludes_cards_present_in_a_decklist() {
        let (_dir, db) = fixture_db();
        let decklist = "Commander\n1 Doom Blade\n\nDeck\n1 Rampant Growth\n";
        let exclude_names = names_in_decklist_text(decklist);
        let results = search(&db, &[], Some(&exclude_names));
        let names: Vec<_> = results.iter().map(|c| c.name.as_str()).collect();
        assert!(!names.contains(&"Doom Blade"), "commander excluded");
        assert!(!names.contains(&"Rampant Growth"), "deck card excluded");
        assert!(names.contains(&"Beast Within"), "other cards kept");
    }

    #[test]
    fn names_in_decklist_reads_a_file_with_the_same_parsing_as_analyze() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("deck.txt");
        std::fs::write(&path, "Commander\n1 Doom Blade\n\nDeck\n1 Rampant Growth\n").unwrap();
        let names = names_in_decklist(path.to_str().unwrap()).unwrap();
        assert!(names.contains("Doom Blade"));
        assert!(names.contains("Rampant Growth"));
    }

    /// Reproduit l'exemple des critères d'acceptation de l'issue #37 :
    /// `kb search --role removal_cible --color-identity BG --legal-in
    /// commander --mana-value-max 3`.
    #[test]
    fn combines_role_color_identity_legal_in_and_mana_value_max() {
        let (_dir, db) = fixture_db();
        let results = search(
            &db,
            &[
                "--role",
                "removal_cible",
                "--color-identity",
                "BG",
                "--legal-in",
                "commander",
                "--mana-value-max",
                "3",
            ],
            None,
        );
        let names: Vec<_> = results.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            {
                let mut n = names.clone();
                n.sort_unstable();
                n
            },
            vec!["Beast Within", "Doom Blade"]
        );
        assert!(
            !names.contains(&"Terminate"),
            "color identity has R, not in BG"
        );
        assert!(
            !names.contains(&"Damnation"),
            "wipe, not removal_cible, and mana value 4 > 3"
        );
        assert!(
            !names.contains(&"Hypothetical Banned Removal"),
            "not legal in commander"
        );
    }

    #[test]
    fn role_theme_and_legality_filters_see_corrections() {
        let (_dir, db) = fixture_db();
        let db = db.with_corrections(Corrections::from([
            (
                "Vanilla Bear".to_string(),
                CardCorrections {
                    roles: Some(vec!["ramp".to_string()]),
                    themes: Some(vec!["tokens".to_string()]),
                    ..Default::default()
                },
            ),
            (
                "Hypothetical Banned Removal".to_string(),
                CardCorrections {
                    legal_in_commander: Some(true),
                    ..Default::default()
                },
            ),
            (
                "Doom Blade".to_string(),
                CardCorrections {
                    legal_in_commander: Some(false),
                    ..Default::default()
                },
            ),
        ]));
        let names = |results: Vec<Card>| -> Vec<String> {
            let mut names: Vec<String> = results.into_iter().map(|c| c.name).collect();
            names.sort();
            names
        };
        assert_eq!(
            names(search(&db, &["--role", "ramp"], None)),
            vec!["Rampant Growth", "Vanilla Bear"]
        );
        assert_eq!(
            names(search(&db, &["--theme", "tokens"], None)),
            vec!["Beast Within", "Vanilla Bear"]
        );
        assert_eq!(
            names(search(
                &db,
                &[
                    "--role",
                    "removal_cible",
                    "--color-identity",
                    "B",
                    "--legal-in",
                    "commander",
                ],
                None,
            )),
            vec!["Hypothetical Banned Removal"]
        );
    }

    #[test]
    fn post_filter_results_are_truncated_to_the_requested_limit() {
        let (_dir, db) = fixture_db();
        // Doom Blade et Hypothetical Banned Removal (si on ignore la
        // légalité) partagent le Rôle removal_cible dans l'identité B :
        // ici on ne filtre que par Rôle, sans légalité, pour avoir plus
        // d'un résultat, et on vérifie que `limit` s'applique bien après
        // le filtre post-SQL plutôt que sur le pool intermédiaire.
        let results = search(&db, &["--role", "removal_cible", "--limit", "1"], None);
        assert_eq!(results.len(), 1);
    }
}
