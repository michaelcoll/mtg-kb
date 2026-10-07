//! Tout ce que le Rapport d'analyse doit à la Base cartes : validation des
//! Suggestions et Impressions de référence des Cartes affichées. `render` ne
//! reçoit plus que le `ReportModel` qui en résulte.

use std::collections::{HashMap, HashSet};
use std::fmt;

use super::swap::{SwapWarning, swap_warnings};
use super::validate::validate_suggestions;
use crate::analyze::metrics::detect_roles;
use crate::analyze::origins::origins_of;
use crate::db::cards::CardsDb;
use crate::model::Origin;
use crate::model::{AnalyzeResult, Card, EnrichedAnalysis, ReferencePrinting, Verdict};

/// Ce que `prepare` lit de la Base cartes (Corrections comprises, ADR 0005).
pub trait PrintingLookup {
    fn card(&self, name: &str) -> anyhow::Result<Option<Card>>;
    fn reference_printing(&self, name: &str) -> anyhow::Result<Option<ReferencePrinting>>;
}

impl PrintingLookup for CardsDb {
    fn card(&self, name: &str) -> anyhow::Result<Option<Card>> {
        CardsDb::card(self, name)
    }

    fn reference_printing(&self, name: &str) -> anyhow::Result<Option<ReferencePrinting>> {
        CardsDb::reference_printing(self, name)
    }
}

/// Impressions de référence des Cartes citées par leur seul nom (Synergies,
/// annexe), par nom de Carte.
pub type CardPrintings = HashMap<String, ReferencePrinting>;

/// Une Suggestion validée, avec ses Origines et les Impressions de référence
/// de sa Carte et de sa Carte à retirer.
#[derive(Debug, Clone, PartialEq)]
pub struct ValidatedSuggestion {
    pub card_name: String,
    pub justification: String,
    pub card_to_remove: Option<String>,
    pub printing: Option<ReferencePrinting>,
    pub card_to_remove_printing: Option<ReferencePrinting>,
    pub origins: Vec<Origin>,
    /// Rôles à seuil que l'échange affaiblit ; n'empêche pas le Rapport.
    pub swap_warnings: Vec<SwapWarning>,
}

/// Entrée de `render` : Suggestions validées et Impressions de référence
/// résolues.
#[derive(Debug, Clone)]
pub struct ReportModel {
    pub analysis: AnalyzeResult,
    pub verdict: Verdict,
    pub commander_printing: Option<ReferencePrinting>,
    pub suggestions: Vec<ValidatedSuggestion>,
    pub card_printings: CardPrintings,
}

#[derive(Debug)]
pub enum PrepareError {
    /// Suggestions invalides : aucun Rapport d'analyse ne doit être écrit.
    Violations(Vec<String>),
    /// Erreur de lecture de la Base cartes.
    Lookup(anyhow::Error),
}

impl fmt::Display for PrepareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrepareError::Violations(violations) => write!(
                f,
                "Suggestions invalides, aucun rapport écrit :\n{}",
                violations.join("\n")
            ),
            PrepareError::Lookup(_) => write!(f, "lecture de la Base cartes"),
        }
    }
}

impl std::error::Error for PrepareError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PrepareError::Violations(_) => None,
            PrepareError::Lookup(e) => Some(e.as_ref()),
        }
    }
}

impl From<anyhow::Error> for PrepareError {
    fn from(e: anyhow::Error) -> Self {
        PrepareError::Lookup(e)
    }
}

/// Cartes affichées par leur seul nom avec vignette au survol : Synergies et
/// annexe des Recommandations externes.
fn hover_card_names(analysis: &AnalyzeResult) -> HashSet<&str> {
    let synergies = analysis
        .synergies
        .iter()
        .flat_map(|s| s.cards.iter().map(String::as_str));
    let edhrec = analysis
        .external
        .edhrec_recommendations
        .iter()
        .map(|r| r.card.name.as_str());
    let recommander = analysis
        .external
        .recommander_recommendations
        .iter()
        .map(|r| r.card.name.as_str());
    synergies.chain(edhrec).chain(recommander).collect()
}

/// Valide les Suggestions (via `DeckContext`) puis résout les Impressions de
/// référence de toutes les Cartes affichées.
pub fn prepare(
    enriched: EnrichedAnalysis,
    lookup: &dyn PrintingLookup,
) -> Result<ReportModel, PrepareError> {
    let violations = validate_suggestions(&enriched, lookup)?;
    if !violations.is_empty() {
        return Err(PrepareError::Violations(violations));
    }

    let EnrichedAnalysis {
        analysis,
        verdict,
        suggestions,
    } = enriched;

    let commander_printing = lookup.reference_printing(&analysis.commander.name)?;
    let suggestions = suggestions
        .into_iter()
        .map(|s| {
            let printing = lookup.reference_printing(&s.card_name)?;
            let (card_to_remove_printing, swap_warnings) = match &s.card_to_remove {
                Some(name) => {
                    // Validée plus haut : la Suggestion est une Carte connue.
                    let suggestion_roles = lookup
                        .card(&s.card_name)?
                        .map(|card| detect_roles(&card))
                        .unwrap_or_default();
                    (
                        lookup.reference_printing(name)?,
                        swap_warnings(&analysis, &suggestion_roles, name),
                    )
                }
                None => (None, vec![]),
            };
            Ok(ValidatedSuggestion {
                swap_warnings,
                origins: origins_of(&analysis, &s.card_name),
                card_name: s.card_name,
                justification: s.justification,
                card_to_remove: s.card_to_remove,
                printing,
                card_to_remove_printing,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let mut card_printings = CardPrintings::new();
    for name in hover_card_names(&analysis) {
        if let Some(printing) = lookup.reference_printing(name)? {
            card_printings.insert(name.to_string(), printing);
        }
    }

    Ok(ReportModel {
        analysis,
        verdict,
        commander_printing,
        suggestions,
        card_printings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    use std::collections::BTreeMap;

    /// Base cartes en mémoire : une Impression de référence par Carte connue.
    #[derive(Default)]
    struct MapLookup {
        cards: HashMap<String, Card>,
        printings: HashMap<String, ReferencePrinting>,
        failing: bool,
    }

    impl MapLookup {
        fn with(mut self, card: Card) -> Self {
            let id = format!("{}-id", super::super::slugify(&card.name));
            self.printings.insert(
                card.name.clone(),
                ReferencePrinting {
                    scryfall_id: id,
                    set_code: "TST".to_string(),
                    number: "1".to_string(),
                    is_two_faced: false,
                },
            );
            self.cards.insert(card.name.clone(), card);
            self
        }
    }

    impl PrintingLookup for MapLookup {
        fn card(&self, name: &str) -> anyhow::Result<Option<Card>> {
            if self.failing {
                anyhow::bail!("base cartes illisible");
            }
            Ok(self.cards.get(name).cloned())
        }

        fn reference_printing(&self, name: &str) -> anyhow::Result<Option<ReferencePrinting>> {
            if self.failing {
                anyhow::bail!("base cartes illisible");
            }
            Ok(self.printings.get(name).cloned())
        }
    }

    fn green(name: &str) -> Card {
        Card::named(name, &["G"])
    }

    fn lookup() -> MapLookup {
        MapLookup::default()
            .with(Card::named(
                "Atraxa, Praetors' Voice",
                &["B", "G", "U", "W"],
            ))
            .with(green("Llanowar Elves"))
            .with(green("Rampant Growth"))
            .with(green("Cultivate"))
            .with(green("Beast Within"))
            .with(green("Krenko, Mob Boss"))
            .with(Card::named("Sol Ring", &[]))
            .with(Card::named("Lightning Bolt", &["R"]))
    }

    fn suggestion(name: &str, card_to_remove: Option<&str>) -> Suggestion {
        Suggestion {
            card_name: name.to_string(),
            justification: format!("pourquoi {name}"),
            card_to_remove: card_to_remove.map(str::to_string),
        }
    }

    fn sample(suggestions: Vec<Suggestion>) -> EnrichedAnalysis {
        EnrichedAnalysis {
            analysis: AnalyzeResult {
                commander: Card::named("Atraxa, Praetors' Voice", &["B", "G", "U", "W"]),
                cards: vec![ResolvedCard {
                    quantity: 1,
                    roles: vec![],
                    themes: vec![],
                    card: green("Llanowar Elves"),
                }],
                unresolved: vec![],
                card_count: 100,
                bracket: None,
                construction_errors: vec![],
                mana_curve: ManaCurve {
                    buckets: vec![],
                    average_mana_value: 2.5,
                },
                mana_base: ManaBase {
                    land_count: 37,
                    sources_by_color: BTreeMap::new(),
                    symbols_by_color: BTreeMap::new(),
                },
                role_counts: BTreeMap::new(),
                thresholds: Default::default(),
                weaknesses: vec![],
                synergies: vec![Synergy {
                    theme: "tokens".to_string(),
                    cards: vec!["Krenko, Mob Boss".to_string()],
                }],
                candidates: vec![Candidate {
                    card: green("Rampant Growth"),
                    score: 2,
                    matched_themes: vec![],
                    matched_weak_roles: vec!["ramp".to_string()],
                    origins: vec![],
                }],
                external: ExternalSources {
                    edhrec_recommendations: vec![EdhrecRecommendation {
                        card: Card::named("Sol Ring", &[]),
                        synergy: 0.4,
                        inclusion_rate: 0.9,
                        header: "Top Cards".to_string(),
                        origins: vec![],
                    }],
                    recommander_recommendations: vec![RecommanderRecommendation {
                        card: green("Cultivate"),
                        score: 3.0,
                        origins: vec![],
                    }],
                    ..ExternalSources::default()
                },
            },
            verdict: Verdict {
                summary: "Solide".to_string(),
                strengths: vec![],
                weaknesses: vec![],
                priorities: vec![],
            },
            suggestions,
        }
    }

    fn printing_of(name: &str) -> Option<ReferencePrinting> {
        lookup().printings.get(name).cloned()
    }

    #[test]
    fn each_suggestion_carries_its_own_origins_and_printings() {
        let enriched = sample(vec![
            suggestion("Beast Within", None),
            suggestion("Rampant Growth", Some("Llanowar Elves")),
            suggestion("Cultivate", None),
        ]);
        let model = prepare(enriched, &lookup()).unwrap();

        let summary: Vec<(&str, &[Origin])> = model
            .suggestions
            .iter()
            .map(|s| (s.card_name.as_str(), s.origins.as_slice()))
            .collect();
        assert_eq!(
            summary,
            vec![
                ("Beast Within", &[Origin::Investigation][..]),
                ("Rampant Growth", &[Origin::Kb][..]),
                ("Cultivate", &[Origin::Recommander][..]),
            ]
        );

        let rampant = &model.suggestions[1];
        assert_eq!(rampant.printing, printing_of("Rampant Growth"));
        assert_eq!(rampant.justification, "pourquoi Rampant Growth");
        assert_eq!(rampant.card_to_remove.as_deref(), Some("Llanowar Elves"));
        assert_eq!(
            rampant.card_to_remove_printing,
            printing_of("Llanowar Elves")
        );
        assert_eq!(model.suggestions[0].card_to_remove_printing, None);
    }

    #[test]
    fn every_hover_card_gets_its_reference_printing() {
        let model = prepare(sample(vec![]), &lookup()).unwrap();
        assert_eq!(
            model.commander_printing,
            printing_of("Atraxa, Praetors' Voice")
        );
        for name in ["Krenko, Mob Boss", "Sol Ring", "Cultivate"] {
            assert_eq!(
                model.card_printings.get(name).cloned(),
                printing_of(name),
                "{name}"
            );
        }

        let html = super::super::render(&model);
        for id in ["krenko-mob-boss-id", "sol-ring-id", "cultivate-id"] {
            assert!(
                html.contains(&format!(
                    r#"data-hover-img="https://api.scryfall.com/cards/{id}?"#
                )),
                "{id}"
            );
        }
    }

    #[test]
    fn a_card_without_reference_printing_is_simply_absent() {
        let mut lookup = lookup();
        lookup.printings.remove("Sol Ring");
        let model = prepare(sample(vec![]), &lookup).unwrap();
        assert!(!model.card_printings.contains_key("Sol Ring"));
    }

    #[test]
    fn invalid_suggestions_are_violations() {
        let enriched = sample(vec![
            suggestion("Lightning Bolt", None),
            suggestion("Not A Real Card", None),
        ]);
        match prepare(enriched, &lookup()) {
            Err(PrepareError::Violations(violations)) => {
                assert_eq!(violations.len(), 2);
                assert!(violations[0].contains("Lightning Bolt"));
                assert!(violations[1].contains("Not A Real Card"));
            }
            other => panic!("violations attendues, obtenu {other:?}"),
        }
    }

    fn game_changer(name: &str) -> Card {
        Card {
            game_changer: true,
            ..green(name)
        }
    }

    /// JSON enrichi relu avec `bracket` et `game_changers_in_deck` Game
    /// Changers dans le Deck, comme `kb report` le lit.
    fn enriched_json_in_bracket(
        bracket: u8,
        game_changers_in_deck: &[&str],
        suggestions: Vec<Suggestion>,
    ) -> EnrichedAnalysis {
        let mut enriched = sample(suggestions);
        enriched
            .analysis
            .cards
            .extend(game_changers_in_deck.iter().map(|name| ResolvedCard {
                quantity: 1,
                roles: vec![],
                themes: vec![],
                card: game_changer(name),
            }));
        let mut json = serde_json::to_value(&enriched).unwrap();
        json["bracket"] = bracket.into();
        serde_json::from_value(json).unwrap()
    }

    fn lookup_with_game_changers() -> MapLookup {
        lookup().with(game_changer("Survival of the Fittest"))
    }

    #[test]
    fn a_game_changer_suggestion_beyond_the_bracket_limit_is_refused() {
        let enriched =
            enriched_json_in_bracket(2, &[], vec![suggestion("Survival of the Fittest", None)]);
        match prepare(enriched, &lookup_with_game_changers()) {
            Err(PrepareError::Violations(violations)) => {
                assert_eq!(violations.len(), 1, "{violations:?}");
                assert!(
                    violations[0].contains("Survival of the Fittest"),
                    "{violations:?}"
                );
                assert!(violations[0].contains("Game Changer"), "{violations:?}");
                assert!(violations[0].contains("Bracket 2"), "{violations:?}");
            }
            other => panic!("violation attendue, obtenu {other:?}"),
        }
    }

    #[test]
    fn in_bracket_3_a_game_changer_suggestion_is_refused_only_from_three_in_the_deck() {
        let suggest = || vec![suggestion("Survival of the Fittest", None)];
        let below = enriched_json_in_bracket(3, &["Demonic Tutor", "Cyclonic Rift"], suggest());
        assert!(prepare(below, &lookup_with_game_changers()).is_ok());

        let at_limit = enriched_json_in_bracket(
            3,
            &["Demonic Tutor", "Cyclonic Rift", "Smothering Tithe"],
            suggest(),
        );
        assert!(matches!(
            prepare(at_limit, &lookup_with_game_changers()),
            Err(PrepareError::Violations(_))
        ));
    }

    #[test]
    fn without_bracket_a_game_changer_suggestion_is_accepted() {
        let enriched = sample(vec![suggestion("Survival of the Fittest", None)]);
        assert!(prepare(enriched, &lookup_with_game_changers()).is_ok());
    }

    fn with_roles(mut card: Card, roles: &[&str]) -> Card {
        card.corrections.roles = Some(roles.iter().map(|r| r.to_string()).collect());
        card
    }

    /// Deck avec exactement `min_ramp` (10) ramps, dont Llanowar Elves, et
    /// une Carte sans Rôle (Krenko) ; Rampant Growth est un ramp hors Deck.
    fn deck_at_ramp_threshold(suggestions: Vec<Suggestion>) -> (EnrichedAnalysis, MapLookup) {
        let mut enriched = sample(suggestions);
        enriched.analysis.cards = vec![
            ResolvedCard {
                quantity: 1,
                roles: vec!["ramp".to_string()],
                themes: vec![],
                card: green("Llanowar Elves"),
            },
            ResolvedCard {
                quantity: 1,
                roles: vec![],
                themes: vec![],
                card: green("Krenko, Mob Boss"),
            },
        ];
        enriched.analysis.role_counts = BTreeMap::from([
            ("ramp".to_string(), 10),
            ("pioche".to_string(), 8),
            ("removal_cible".to_string(), 8),
            ("wipe".to_string(), 2),
            ("terrain".to_string(), 37),
        ]);
        let lookup = lookup().with(with_roles(green("Rampant Growth"), &["ramp"]));
        (enriched, lookup)
    }

    fn swap_warnings_of(model: &ReportModel) -> Vec<(&str, Vec<SwapWarning>)> {
        model
            .suggestions
            .iter()
            .map(|s| (s.card_name.as_str(), s.swap_warnings.clone()))
            .collect()
    }

    #[test]
    fn removing_the_only_ramp_above_threshold_warns_with_the_count_after_swap() {
        let (enriched, lookup) =
            deck_at_ramp_threshold(vec![suggestion("Beast Within", Some("Llanowar Elves"))]);
        let model = prepare(enriched, &lookup).unwrap();

        assert_eq!(
            swap_warnings_of(&model),
            vec![(
                "Beast Within",
                vec![SwapWarning {
                    role: "ramp".to_string(),
                    count_after: 9,
                    minimum: 10,
                }]
            )]
        );
        let html = super::super::render(&model);
        assert!(html.contains("ramp"));
        assert!(html.contains("class=\"swap-warning\""));
        assert!(html.contains("9"));
    }

    #[test]
    fn a_swap_keeping_every_role_at_its_minimum_does_not_warn() {
        let (enriched, lookup) = deck_at_ramp_threshold(vec![
            // ramp contre ramp : 10 → 10.
            suggestion("Rampant Growth", Some("Llanowar Elves")),
            // aucune Carte à Rôle retirée.
            suggestion("Beast Within", Some("Krenko, Mob Boss")),
        ]);
        let model = prepare(enriched, &lookup).unwrap();

        assert_eq!(
            swap_warnings_of(&model),
            vec![("Rampant Growth", vec![]), ("Beast Within", vec![])]
        );
        assert!(!super::super::render(&model).contains("class=\"swap-warning\""));
    }

    #[test]
    fn a_role_already_below_threshold_warns_only_if_it_drops_further() {
        let (mut enriched, lookup) = deck_at_ramp_threshold(vec![
            suggestion("Rampant Growth", Some("Krenko, Mob Boss")),
            suggestion("Beast Within", Some("Krenko, Mob Boss")),
            suggestion("Cultivate", Some("Llanowar Elves")),
        ]);
        enriched.analysis.role_counts.insert("ramp".to_string(), 5);
        let model = prepare(enriched, &lookup).unwrap();

        assert_eq!(
            swap_warnings_of(&model),
            vec![
                ("Rampant Growth", vec![]),
                ("Beast Within", vec![]),
                (
                    "Cultivate",
                    vec![SwapWarning {
                        role: "ramp".to_string(),
                        count_after: 4,
                        minimum: 10,
                    }]
                ),
            ]
        );
    }

    #[test]
    fn a_suggestion_without_card_to_remove_never_warns() {
        let (mut enriched, lookup) = deck_at_ramp_threshold(vec![suggestion("Beast Within", None)]);
        enriched.analysis.role_counts.clear();
        let model = prepare(enriched, &lookup).unwrap();

        assert_eq!(swap_warnings_of(&model), vec![("Beast Within", vec![])]);
    }

    #[test]
    fn the_swap_is_judged_against_the_thresholds_recorded_in_the_analysis() {
        let (mut enriched, lookup) =
            deck_at_ramp_threshold(vec![suggestion("Beast Within", Some("Llanowar Elves"))]);
        enriched.analysis.thresholds.min_ramp = 9;
        let model = prepare(enriched, &lookup).unwrap();

        assert_eq!(swap_warnings_of(&model), vec![("Beast Within", vec![])]);
    }

    #[test]
    fn a_cards_db_error_is_not_a_violation() {
        let failing = MapLookup {
            failing: true,
            ..lookup()
        };
        let enriched = sample(vec![suggestion("Rampant Growth", None)]);
        match prepare(enriched, &failing) {
            Err(PrepareError::Lookup(e)) => assert!(e.to_string().contains("illisible")),
            other => panic!("erreur de Base cartes attendue, obtenu {other:?}"),
        }
    }
}
