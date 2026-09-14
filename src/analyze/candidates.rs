use std::collections::HashSet;

use anyhow::Result;

use crate::db::cards::{CardsDb, SearchFilters};
use crate::model::Candidate;

use super::ranking::sort_desc_by_score_then_name;
use super::themes;

/// Nombre maximal de Candidats retenus par Rôle sous-représenté et par Thème
/// majeur (voir `find_candidates`).
const CANDIDATE_LIMIT_PER_BUCKET: usize = 10;

/// Cartes légales Commander, dans l'Identité de couleur du Commandant,
/// absentes du Deck : le pool complet (sans plafond ni tri alphabétique) est
/// scoré en Rust via la détection de Rôles/Thèmes existante — une seule fois
/// par Carte, pas une fois par Rôle/Thème — puis réparti en Candidats :
/// jusqu'à `CANDIDATE_LIMIT_PER_BUCKET` par Rôle sous-représenté et jusqu'à
/// `CANDIDATE_LIMIT_PER_BUCKET` par Thème majeur, dédupliqués par nom. Un
/// Candidat porte tous les Rôles/Thèmes qu'il matche parmi ceux du Deck
/// (`matched_themes` / `matched_weak_roles`), pas seulement celui qui l'a
/// fait retenir dans son panier. Seules les Cartes qui correspondent à au
/// moins un Thème majeur ou un Rôle en Point faible sont retenues : un score
/// de 0 n'apporterait rien de plus qu'un `kb search`.
pub fn find_candidates(
    db: &CardsDb,
    color_identity: &[String],
    deck_names: &HashSet<String>,
    major_themes: &HashSet<String>,
    weak_roles: &[String],
) -> Result<Vec<Candidate>> {
    let pool = db.search(&SearchFilters {
        legal_in_format: Some("commander".to_string()),
        color_identity_subset_of: Some(color_identity.to_vec()),
        limit: 0,
        ..Default::default()
    })?;

    let scored: Vec<Candidate> = pool
        .into_iter()
        .filter(|card| !deck_names.contains(&card.name))
        .filter_map(|card| {
            let card_themes = themes::detect_themes(&card);
            let matched_themes: Vec<String> = card_themes
                .into_iter()
                .filter(|t| major_themes.contains(t))
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

    let mut sorted_themes: Vec<&String> = major_themes.iter().collect();
    sorted_themes.sort();

    let mut candidates: Vec<Candidate> = Vec::new();
    let mut picked: HashSet<String> = HashSet::new();

    for role in weak_roles {
        for candidate in top_matches(&scored, |c| c.matched_weak_roles.iter().any(|r| r == role)) {
            if picked.insert(candidate.card.name.clone()) {
                candidates.push(candidate.clone());
            }
        }
    }
    for theme in sorted_themes {
        for candidate in top_matches(&scored, |c| c.matched_themes.iter().any(|t| t == theme)) {
            if picked.insert(candidate.card.name.clone()) {
                candidates.push(candidate.clone());
            }
        }
    }

    sort_desc_by_score_then_name(&mut candidates, |c| c.score as f64, |c| &c.card.name);
    Ok(candidates)
}

/// Les `CANDIDATE_LIMIT_PER_BUCKET` meilleurs Candidats scorés qui vérifient
/// `matches`, classés par score puis par nom (comme le tri final).
fn top_matches(scored: &[Candidate], matches: impl Fn(&Candidate) -> bool) -> Vec<&Candidate> {
    let mut ranked: Vec<&Candidate> = scored.iter().filter(|c| matches(c)).collect();
    sort_desc_by_score_then_name(&mut ranked, |c| c.score as f64, |c| &c.card.name);
    ranked.truncate(CANDIDATE_LIMIT_PER_BUCKET);
    ranked
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

    /// Base cartes de plus de 500 Cartes légales Commander mono-vert,
    /// couvrant tout l'alphabet, plus une Carte "Zephyr Ramp Elemental" en
    /// fin d'alphabet — pour prouver que le pool n'est plus tronqué à un
    /// plafond alphabétique (issue #36). Quand `bulk_matches` est faux, les
    /// Cartes en volume sont de simples vanilles (aucun Rôle/Thème détecté,
    /// donc score 0) : seule la Carte "Z…" matche, ce qui isole la preuve de
    /// portée sans dépendre du tri à égalité de score. Quand `bulk_matches`
    /// est vrai, toutes les Cartes (dont "Z…") sont des sources de ramp
    /// Elfes, pour les tests de plafond par panier.
    fn large_fixture_db(card_count: usize, bulk_matches: bool) -> (tempfile::TempDir, CardsDb) {
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
            "#,
        )
        .unwrap();

        let letters: Vec<char> = ('A'..='Z').collect();
        for i in 0..card_count {
            let letter = letters[i % letters.len()];
            let uuid = format!("card-{i}");
            let name = format!("{letter} Mana Dork {i}");
            if bulk_matches {
                conn.execute(
                    "INSERT INTO cards VALUES (?1, ?2, '{G}', 1.0, 'Creature', 'Creature', 'Elf', NULL, \
                     '{T}: Add {G}.', 'G', 'G', NULL, '1', '1', NULL)",
                    rusqlite::params![uuid, name],
                )
                .unwrap();
            } else {
                conn.execute(
                    "INSERT INTO cards VALUES (?1, ?2, '{1}{G}', 2.0, 'Creature', 'Creature', 'Bear', NULL, \
                     '', 'G', 'G', NULL, '2', '2', NULL)",
                    rusqlite::params![uuid, name],
                )
                .unwrap();
            }
            conn.execute(
                "INSERT INTO cardLegalities VALUES (?1, 'Legal')",
                rusqlite::params![uuid],
            )
            .unwrap();
        }
        // Une Carte nommément en "Z..." pour un test explicite et lisible :
        // toujours une source de ramp Elfe, qu'elle se distingue (bulk non
        // matchant) ou se fonde dans la masse (bulk matchant).
        conn.execute(
            "INSERT INTO cards VALUES ('z-card', 'Zephyr Ramp Elemental', '{G}', 1.0, 'Creature', 'Creature', \
             'Elf', NULL, '{T}: Add {G}.', 'G', 'G', NULL, '1', '1', NULL)",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO cardLegalities VALUES ('z-card', 'Legal')", [])
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
        let mut major_themes = HashSet::new();
        major_themes.insert("tokens".to_string());

        let candidates = find_candidates(
            &db,
            &color_identity,
            &HashSet::new(),
            &major_themes,
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

    #[test]
    fn a_late_alphabet_card_can_be_a_candidate_in_a_500_plus_card_pool() {
        let (_dir, db) = large_fixture_db(520, false);
        let color_identity = vec!["G".to_string()];

        let candidates = find_candidates(
            &db,
            &color_identity,
            &HashSet::new(),
            &HashSet::new(),
            &["ramp".to_string()],
        )
        .unwrap();

        assert!(
            candidates
                .iter()
                .any(|c| c.card.name == "Zephyr Ramp Elemental"),
            "a card starting with a late letter must be reachable, not just the alphabetical head; got {} candidates",
            candidates.len()
        );
    }

    #[test]
    fn caps_at_ten_candidates_per_weak_role_without_duplicates() {
        let (_dir, db) = large_fixture_db(520, true);
        let color_identity = vec!["G".to_string()];

        let candidates = find_candidates(
            &db,
            &color_identity,
            &HashSet::new(),
            &HashSet::new(),
            &["ramp".to_string()],
        )
        .unwrap();

        assert_eq!(
            candidates.len(),
            CANDIDATE_LIMIT_PER_BUCKET,
            "every fixture card matches the single weak role \"ramp\", capped at 10"
        );
        let unique: HashSet<&str> = candidates.iter().map(|c| c.card.name.as_str()).collect();
        assert_eq!(unique.len(), candidates.len(), "no duplicate candidates");
    }

    #[test]
    fn caps_at_ten_candidates_per_major_theme_without_duplicates() {
        let (_dir, db) = large_fixture_db(520, true);
        let color_identity = vec!["G".to_string()];
        // Toutes les Cartes de la fixture (y compris "Z...") sont des Elfes :
        // "tribal:Elf" est ici traité comme Thème majeur pour vérifier le
        // plafond par Thème, indépendamment du plafond par Rôle.
        let mut major_themes = HashSet::new();
        major_themes.insert("tribal:Elf".to_string());

        let candidates =
            find_candidates(&db, &color_identity, &HashSet::new(), &major_themes, &[]).unwrap();

        assert_eq!(candidates.len(), CANDIDATE_LIMIT_PER_BUCKET);
        let unique: HashSet<&str> = candidates.iter().map(|c| c.card.name.as_str()).collect();
        assert_eq!(unique.len(), candidates.len(), "no duplicate candidates");
    }

    #[test]
    fn weak_role_and_major_theme_buckets_are_deduplicated_in_the_union() {
        let (_dir, db) = large_fixture_db(520, true);
        let color_identity = vec!["G".to_string()];
        let mut major_themes = HashSet::new();
        major_themes.insert("tribal:Elf".to_string());

        // Chaque Carte Elfe de la fixture matche à la fois le Rôle "ramp" et
        // le Thème "tribal:Elf" : les deux paniers de 10 se recouvrent
        // entièrement, donc l'union dédupliquée ne doit pas dépasser 10.
        let candidates = find_candidates(
            &db,
            &color_identity,
            &HashSet::new(),
            &major_themes,
            &["ramp".to_string()],
        )
        .unwrap();

        assert_eq!(
            candidates.len(),
            CANDIDATE_LIMIT_PER_BUCKET,
            "same cards fill both buckets: union must dedupe, not add up to 20"
        );
        for candidate in &candidates {
            assert_eq!(candidate.matched_weak_roles, vec!["ramp".to_string()]);
            assert_eq!(candidate.matched_themes, vec!["tribal:Elf".to_string()]);
        }
    }
}
