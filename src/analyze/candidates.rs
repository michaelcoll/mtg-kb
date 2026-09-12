use std::collections::HashSet;

use anyhow::Result;

use crate::db::cards::{CardsDb, SearchFilters};
use crate::model::Candidate;

use super::themes;

const CANDIDATE_LIMIT: usize = 20;
const FETCH_CAP: usize = 500;

/// Cartes légales Commander, dans l'Identité de couleur du Commandant,
/// absentes du Deck, classées par correspondance avec les Thèmes du Deck et
/// les Rôles en Point faible. Seules les Cartes qui correspondent à au
/// moins un Thème ou un Rôle en Point faible sont retenues : un score de 0
/// n'apporterait rien de plus qu'un `kb search`.
pub fn find_candidates(
    db: &CardsDb,
    color_identity: &[String],
    deck_names: &HashSet<String>,
    deck_themes: &HashSet<String>,
    weak_roles: &[String],
) -> Result<Vec<Candidate>> {
    let pool = db.search(&SearchFilters {
        legal_in_format: Some("commander".to_string()),
        color_identity_subset_of: Some(color_identity.to_vec()),
        limit: FETCH_CAP,
        ..Default::default()
    })?;

    let mut candidates: Vec<Candidate> = pool
        .into_iter()
        .filter(|card| !deck_names.contains(&card.name))
        .filter_map(|card| {
            let card_themes = themes::detect_themes(&card);
            let matched_themes: Vec<String> = card_themes
                .into_iter()
                .filter(|t| deck_themes.contains(t))
                .collect();

            let card_roles = super::metrics::detect_roles(&card);
            let matched_weak_roles: Vec<String> = card_roles
                .into_iter()
                .filter(|r| weak_roles.contains(r))
                .collect();

            let score = (matched_themes.len() as u32) + 2 * (matched_weak_roles.len() as u32);
            if score == 0 {
                return None;
            }
            Some(Candidate {
                card,
                score,
                matched_themes,
                matched_weak_roles,
            })
        })
        .collect();

    candidates.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.card.name.cmp(&b.card.name))
    });
    candidates.truncate(CANDIDATE_LIMIT);
    Ok(candidates)
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

            INSERT INTO cards VALUES ('rampant', 'Rampant Growth', '{1}{G}', 2.0, 'Sorcery', 'Sorcery',
                NULL, NULL, 'Search your library for a basic land card and put it onto the battlefield tapped.',
                'G', 'G', NULL, NULL, NULL, NULL);
            INSERT INTO cards VALUES ('krenko', 'Krenko, Mob Boss', '{2}{R}{R}', 4.0, 'Legendary Creature', 'Creature',
                'Goblin', 'Legendary', 'Whenever Krenko attacks, create X 1/1 red Goblin creature tokens.',
                'R', 'R', NULL, '3', '3', NULL);
            INSERT INTO cards VALUES ('bear', 'Grizzly Bears', '{1}{G}', 2.0, 'Creature', 'Creature',
                'Bear', NULL, '', 'G', 'G', NULL, '2', '2', NULL);
            INSERT INTO cards VALUES ('offcolor', 'Lightning Bolt', '{R}', 1.0, 'Instant', 'Instant',
                NULL, NULL, 'Deal 3 damage to any target.', 'R', 'R', NULL, NULL, NULL, NULL);

            INSERT INTO cardLegalities VALUES ('rampant', 'Legal');
            INSERT INTO cardLegalities VALUES ('krenko', 'Legal');
            INSERT INTO cardLegalities VALUES ('bear', 'Legal');
            INSERT INTO cardLegalities VALUES ('offcolor', 'Legal');
            "#,
        )
        .unwrap();
        (dir, CardsDb::open(&path).unwrap())
    }

    #[test]
    fn excludes_cards_already_in_deck_and_out_of_color() {
        let (_dir, db) = fixture_db();
        let mut deck_names = HashSet::new();
        deck_names.insert("Rampant Growth".to_string());
        let color_identity = vec!["G".to_string()];

        let candidates = find_candidates(
            &db,
            &color_identity,
            &deck_names,
            &HashSet::new(),
            &["ramp".to_string()],
        )
        .unwrap();

        let names: Vec<_> = candidates.iter().map(|c| c.card.name.as_str()).collect();
        assert!(!names.contains(&"Rampant Growth"), "already in deck");
        assert!(
            !names.contains(&"Krenko, Mob Boss"),
            "out of color identity (R)"
        );
        assert!(
            !names.contains(&"Lightning Bolt"),
            "out of color identity (R)"
        );
    }

    #[test]
    fn scores_higher_for_weak_role_than_theme_only() {
        let (_dir, db) = fixture_db();
        let color_identity = vec!["G".to_string(), "R".to_string()];
        let mut deck_themes = HashSet::new();
        deck_themes.insert("tokens".to_string());

        let candidates = find_candidates(
            &db,
            &color_identity,
            &HashSet::new(),
            &deck_themes,
            &["ramp".to_string()],
        )
        .unwrap();

        let rampant = candidates
            .iter()
            .find(|c| c.card.name == "Rampant Growth")
            .expect("ramp candidate present");
        let krenko = candidates
            .iter()
            .find(|c| c.card.name == "Krenko, Mob Boss")
            .expect("tokens candidate present");
        assert!(rampant.score > krenko.score);
        assert_eq!(rampant.matched_weak_roles, vec!["ramp".to_string()]);
        assert_eq!(krenko.matched_themes, vec!["tokens".to_string()]);
    }

    #[test]
    fn excludes_zero_score_candidates() {
        let (_dir, db) = fixture_db();
        let color_identity = vec!["G".to_string()];
        let candidates =
            find_candidates(&db, &color_identity, &HashSet::new(), &HashSet::new(), &[]).unwrap();
        assert!(
            !candidates.iter().any(|c| c.card.name == "Grizzly Bears"),
            "vanilla creature matches nothing, should be excluded"
        );
    }
}
