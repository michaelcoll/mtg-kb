//! Règle d'éligibilité partagée par Candidat, Recommandation externe et
//! Suggestion (CONTEXT.md) : légale en Commander, dans l'Identité de couleur
//! du Commandant, absente du Deck (Commandant compris).

use std::collections::HashSet;

use crate::model::{AnalyzeResult, Card, ColorIdentity};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ineligible {
    Illegal,
    OutsideIdentity,
    AlreadyInDeck,
}

#[derive(Debug, Clone)]
pub struct DeckContext {
    commander_name: String,
    commander_identity: ColorIdentity,
    names: HashSet<String>,
    /// Ordre de la Decklist, Commandant exclu : corps de requête Recommander.
    names_without_basic_lands: Vec<String>,
}

impl DeckContext {
    pub fn new<'a>(commander: &'a Card, deck_cards: impl IntoIterator<Item = &'a Card>) -> Self {
        let deck_cards: Vec<&Card> = deck_cards.into_iter().collect();
        let names = std::iter::once(commander)
            .chain(deck_cards.iter().copied())
            .map(|card| card.name.clone())
            .collect();
        let names_without_basic_lands = deck_cards
            .iter()
            .filter(|card| !card.is_basic_land())
            .map(|card| card.name.clone())
            .collect();
        Self {
            commander_name: commander.name.clone(),
            commander_identity: commander.identity(),
            names,
            names_without_basic_lands,
        }
    }

    pub fn commander_name(&self) -> &str {
        &self.commander_name
    }

    /// Noms des Cartes du Deck hors terrains de base et hors Commandant,
    /// dans l'ordre de la Decklist.
    pub fn names_without_basic_lands(&self) -> &[String] {
        &self.names_without_basic_lands
    }

    pub fn from_analysis(analysis: &AnalyzeResult) -> Self {
        Self::new(&analysis.commander, analysis.cards.iter().map(|c| &c.card))
    }

    /// Commandant compris.
    pub fn contains(&self, name: &str) -> bool {
        self.names.contains(name)
    }

    /// Première règle enfreinte : légalité, Identité de couleur, absence du Deck.
    pub fn eligibility(&self, card: &Card) -> Result<(), Ineligible> {
        if !card.legal_in_commander {
            return Err(Ineligible::Illegal);
        }
        if !self.is_within_identity(card) {
            return Err(Ineligible::OutsideIdentity);
        }
        if self.contains(&card.name) {
            return Err(Ineligible::AlreadyInDeck);
        }
        Ok(())
    }

    pub fn commander_identity(&self) -> &ColorIdentity {
        &self.commander_identity
    }

    pub fn is_within_identity(&self, card: &Card) -> bool {
        card.identity().is_subset_of(&self.commander_identity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atraxa() -> Card {
        Card::named("Atraxa, Praetors' Voice", &["B", "G", "U", "W"])
    }

    #[test]
    fn the_commander_is_already_in_the_deck() {
        let commander = atraxa();
        let context = DeckContext::new(&commander, []);
        assert_eq!(
            context.eligibility(&atraxa()),
            Err(Ineligible::AlreadyInDeck)
        );
    }

    #[test]
    fn a_card_not_legal_in_commander_is_ineligible() {
        let commander = atraxa();
        let context = DeckContext::new(&commander, []);
        let channel = Card {
            legal_in_commander: false,
            ..Card::named("Channel", &["G"])
        };
        assert_eq!(context.eligibility(&channel), Err(Ineligible::Illegal));
    }

    #[test]
    fn a_legal_card_within_the_identity_and_absent_from_the_deck_is_eligible() {
        let commander = atraxa();
        let elves = Card::named("Llanowar Elves", &["G"]);
        let context = DeckContext::new(&commander, [&elves]);
        assert_eq!(
            context.eligibility(&Card::named("Rampant Growth", &["G"])),
            Ok(())
        );
        assert_eq!(context.eligibility(&elves), Err(Ineligible::AlreadyInDeck));
    }

    #[test]
    fn a_card_outside_the_commander_identity_is_ineligible() {
        let commander = atraxa();
        let context = DeckContext::new(&commander, []);
        assert_eq!(
            context.eligibility(&Card::named("Lightning Bolt", &["R"])),
            Err(Ineligible::OutsideIdentity)
        );
    }

    #[test]
    fn exposes_the_commander_name_and_the_deck_names_without_basic_lands_in_order() {
        use crate::model::Face;
        let commander = atraxa();
        let forest = Card::from_face(Face {
            name: "Forest".to_string(),
            types: vec!["Land".to_string()],
            supertypes: vec!["Basic".to_string()],
            ..Face::default()
        });
        let sol_ring = Card::named("Sol Ring", &[]);
        let elves = Card::named("Llanowar Elves", &["G"]);
        let context = DeckContext::new(&commander, [&sol_ring, &forest, &elves]);
        assert_eq!(context.commander_name(), "Atraxa, Praetors' Voice");
        assert_eq!(
            context.names_without_basic_lands(),
            ["Sol Ring".to_string(), "Llanowar Elves".to_string()]
        );
        assert!(
            context.contains("Forest"),
            "le terrain de base reste dans le Deck"
        );
    }

    #[test]
    fn color_identity_comparison_ignores_case() {
        let commander = Card::named("Commander", &["g"]);
        let context = DeckContext::new(&commander, []);
        assert_eq!(
            context.eligibility(&Card::named("Rampant Growth", &["G"])),
            Ok(())
        );
        assert!(
            ColorIdentity::new(&["b".to_string()])
                .is_subset_of(&ColorIdentity::new(&["B".to_string(), "G".to_string()]))
        );
    }
}
