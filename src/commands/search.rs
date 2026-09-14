use std::collections::HashSet;
use std::io::Read as _;

use anyhow::{Context, Result};

use crate::analyze::metrics::detect_roles;
use crate::analyze::themes::detect_themes;
use crate::data_dir::cards_db_path;
use crate::db::cards::{CardsDb, SearchFilters};
use crate::decklist::parser;
use crate::model::Card;
use crate::output::{Format, print_json};

#[allow(clippy::too_many_arguments)]
pub fn run(
    name: Option<String>,
    type_contains: Option<String>,
    subtype_contains: Option<String>,
    text: Vec<String>,
    color_identity: Option<String>,
    legal_in: Option<String>,
    mana_value: Option<f64>,
    mana_value_min: Option<f64>,
    mana_value_max: Option<f64>,
    role: Vec<String>,
    theme: Vec<String>,
    exclude_deck: Option<String>,
    limit: usize,
    format: Format,
) -> Result<()> {
    let db = CardsDb::open(&cards_db_path())?;
    let exclude_names = exclude_deck.as_deref().map(names_in_decklist).transpose()?;

    let results = search_cards(
        &db,
        name,
        type_contains,
        subtype_contains,
        text,
        color_identity,
        legal_in,
        mana_value,
        mana_value_min,
        mana_value_max,
        role,
        theme,
        exclude_names,
        limit,
    )?;

    match format {
        Format::Json => print_json(&results)?,
        Format::Table => print_table(&results),
    }
    Ok(())
}

/// Les filtres Rôle/Thème et l'exclusion par Decklist sont appliqués après
/// la requête SQL, donc `limit` ne s'applique qu'à la fin.
#[allow(clippy::too_many_arguments)]
fn search_cards(
    db: &CardsDb,
    name: Option<String>,
    type_contains: Option<String>,
    subtype_contains: Option<String>,
    text: Vec<String>,
    color_identity: Option<String>,
    legal_in: Option<String>,
    mana_value: Option<f64>,
    mana_value_min: Option<f64>,
    mana_value_max: Option<f64>,
    role: Vec<String>,
    theme: Vec<String>,
    exclude_names: Option<HashSet<String>>,
    limit: usize,
) -> Result<Vec<Card>> {
    let needs_post_filter = !role.is_empty() || !theme.is_empty() || exclude_names.is_some();

    let filters = SearchFilters {
        name,
        type_contains,
        subtype_contains,
        oracle_text_contains: text,
        color_identity_subset_of: color_identity
            .map(|s| s.chars().map(|c| c.to_string()).collect()),
        legal_in_format: legal_in,
        mana_value,
        mana_value_min,
        mana_value_max,
        limit: if needs_post_filter { 0 } else { limit },
    };
    let mut results = db.search(&filters)?;

    if !role.is_empty() {
        results.retain(|card| card_matches_all(&detect_roles(card), &role));
    }
    if !theme.is_empty() {
        results.retain(|card| card_matches_all(&detect_themes(card), &theme));
    }
    if let Some(names) = &exclude_names {
        results.retain(|card| !names.contains(&card.name));
    }
    if needs_post_filter && limit > 0 {
        results.truncate(limit);
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
    let input = if path == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .context("lecture de la Decklist depuis l'entrée standard")?;
        buf
    } else {
        std::fs::read_to_string(path).with_context(|| format!("lecture du fichier {path}"))?
    };
    Ok(names_in_decklist_text(&input))
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
            card.mana_cost.clone().unwrap_or_default(),
            card.type_line.clone().unwrap_or_default()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn fixture_db() -> (tempfile::TempDir, CardsDb) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("AllPrintings.sqlite");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE cards (
                uuid TEXT, name TEXT, manaCost TEXT, manaValue REAL, type TEXT, types TEXT,
                subtypes TEXT, supertypes TEXT, text TEXT, colorIdentity TEXT,
                colors TEXT, keywords TEXT, power TEXT, toughness TEXT, loyalty TEXT
            );
            CREATE TABLE cardLegalities (uuid TEXT, commander TEXT);

            INSERT INTO cards VALUES ('doom-blade', 'Doom Blade', '{1}{B}', 2.0, 'Instant', 'Instant',
                NULL, NULL, 'Destroy target creature.', 'B', 'B', NULL, NULL, NULL, NULL);
            INSERT INTO cards VALUES ('beast-within', 'Beast Within', '{2}{G}', 3.0, 'Sorcery', 'Sorcery',
                NULL, NULL,
                'Destroy target permanent. Its controller creates a 3/3 green Beast creature token.',
                'G', 'G', NULL, NULL, NULL, NULL);
            INSERT INTO cards VALUES ('terminate', 'Terminate', '{B}{R}', 2.0, 'Instant', 'Instant',
                NULL, NULL, 'Destroy target creature or planeswalker.', 'B, R', 'B, R', NULL, NULL, NULL, NULL);
            INSERT INTO cards VALUES ('damnation', 'Damnation', '{2}{B}{B}', 4.0, 'Sorcery', 'Sorcery',
                NULL, NULL, 'Destroy all creatures.', 'B', 'B', NULL, NULL, NULL, NULL);
            INSERT INTO cards VALUES ('banned-removal', 'Hypothetical Banned Removal', '{B}', 1.0,
                'Instant', 'Instant', NULL, NULL, 'Destroy target creature.', 'B', 'B', NULL, NULL, NULL, NULL);
            INSERT INTO cards VALUES ('vanilla-bear', 'Vanilla Bear', '{1}{G}', 2.0, 'Creature', 'Creature',
                'Bear', NULL, '', 'G', 'G', NULL, '2', '2', NULL);
            INSERT INTO cards VALUES ('rampant', 'Rampant Growth', '{1}{G}', 2.0, 'Sorcery', 'Sorcery',
                NULL, NULL,
                'Search your library for a basic land card and put it onto the battlefield tapped.',
                'G', 'G', NULL, NULL, NULL, NULL);
            INSERT INTO cards VALUES ('goblin-king', 'Goblin King', '{1}{R}{R}', 3.0,
                'Creature — Goblin King', 'Creature', 'Goblin', NULL, 'Other Goblins get +1/+1.',
                'R', 'R', NULL, '2', '2', NULL);

            INSERT INTO cardLegalities VALUES ('doom-blade', 'Legal');
            INSERT INTO cardLegalities VALUES ('beast-within', 'Legal');
            INSERT INTO cardLegalities VALUES ('terminate', 'Legal');
            INSERT INTO cardLegalities VALUES ('damnation', 'Legal');
            INSERT INTO cardLegalities VALUES ('banned-removal', 'Banned');
            INSERT INTO cardLegalities VALUES ('vanilla-bear', 'Legal');
            INSERT INTO cardLegalities VALUES ('rampant', 'Legal');
            INSERT INTO cardLegalities VALUES ('goblin-king', 'Legal');
            "#,
        )
        .unwrap();
        (dir, CardsDb::open(&path).unwrap())
    }

    #[allow(clippy::too_many_arguments)]
    fn search(
        db: &CardsDb,
        color_identity: Option<&str>,
        legal_in: Option<&str>,
        mana_value_max: Option<f64>,
        role: &[&str],
        theme: &[&str],
        exclude_names: Option<HashSet<String>>,
        limit: usize,
    ) -> Vec<Card> {
        search_cards(
            db,
            None,
            None,
            None,
            vec![],
            color_identity.map(str::to_string),
            legal_in.map(str::to_string),
            None,
            None,
            mana_value_max,
            role.iter().map(|r| r.to_string()).collect(),
            theme.iter().map(|t| t.to_string()).collect(),
            exclude_names,
            limit,
        )
        .unwrap()
    }

    #[test]
    fn filters_by_role() {
        let (_dir, db) = fixture_db();
        let results = search(&db, None, None, None, &["ramp"], &[], None, 50);
        let names: Vec<_> = results.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["Rampant Growth"]);
    }

    #[test]
    fn filters_by_theme() {
        let (_dir, db) = fixture_db();
        let results = search(&db, None, None, None, &[], &["tribal:Goblin"], None, 50);
        let names: Vec<_> = results.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["Goblin King"]);
    }

    #[test]
    fn excludes_cards_present_in_a_decklist() {
        let (_dir, db) = fixture_db();
        let decklist = "Commander\n1 Doom Blade\n\nDeck\n1 Rampant Growth\n";
        let exclude_names = names_in_decklist_text(decklist);
        let results = search(&db, None, None, None, &[], &[], Some(exclude_names), 50);
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
            Some("BG"),
            Some("commander"),
            Some(3.0),
            &["removal_cible"],
            &[],
            None,
            50,
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
    fn post_filter_results_are_truncated_to_the_requested_limit() {
        let (_dir, db) = fixture_db();
        // Doom Blade et Hypothetical Banned Removal (si on ignore la
        // légalité) partagent le Rôle removal_cible dans l'identité B :
        // ici on ne filtre que par Rôle, sans légalité, pour avoir plus
        // d'un résultat, et on vérifie que `limit` s'applique bien après
        // le filtre post-SQL plutôt que sur le pool intermédiaire.
        let results = search(&db, None, None, None, &["removal_cible"], &[], None, 1);
        assert_eq!(results.len(), 1);
    }
}
