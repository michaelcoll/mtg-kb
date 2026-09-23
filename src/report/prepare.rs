//! Tout ce que le Rapport d'analyse doit à la Base cartes : validation des
//! Suggestions et Impressions de référence des Cartes affichées. `render` ne
//! reçoit plus que le `ReportModel` qui en résulte.

use std::collections::{HashMap, HashSet};
use std::fmt;

use super::origins::{Origin, origins_of};
use super::validate::validate_suggestions;
use crate::db::cards::CardsDb;
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
            let card_to_remove_printing = match &s.card_to_remove {
                Some(name) => lookup.reference_printing(name)?,
                None => None,
            };
            Ok(ValidatedSuggestion {
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
                }],
                external: ExternalSources {
                    edhrec_recommendations: vec![EdhrecRecommendation {
                        card: Card::named("Sol Ring", &[]),
                        synergy: 0.4,
                        inclusion_rate: 0.9,
                        header: "Top Cards".to_string(),
                    }],
                    recommander_recommendations: vec![RecommanderRecommendation {
                        card: green("Cultivate"),
                        score: 3.0,
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
