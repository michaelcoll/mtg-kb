//! Règle d'éligibilité partagée par Candidat, Recommandation externe et
//! Suggestion (CONTEXT.md) : légale en Commander, dans l'Identité de couleur
//! du Commandant, absente du Deck (Commandant compris).

use std::collections::HashSet;

use crate::model::{AnalyzeResult, Card};

/// Identité de couleur, en majuscules par construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorIdentity(Vec<String>);

impl ColorIdentity {
    pub fn new(colors: &[String]) -> Self {
        Self(colors.iter().map(|c| c.to_ascii_uppercase()).collect())
    }

    /// Une lettre par couleur, ex. `"BG"`.
    pub fn from_letters(letters: &str) -> Self {
        Self(
            letters
                .chars()
                .map(|c| c.to_ascii_uppercase().to_string())
                .collect(),
        )
    }

    pub fn is_subset_of(&self, other: &ColorIdentity) -> bool {
        self.0.iter().all(|c| other.0.contains(c))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ineligible {
    Illegal,
    OutsideIdentity,
    AlreadyInDeck,
}

#[derive(Debug, Clone)]
pub struct DeckContext {
    commander_identity: ColorIdentity,
    names: HashSet<String>,
}

impl DeckContext {
    pub fn new<'a>(commander: &'a Card, deck_cards: impl IntoIterator<Item = &'a Card>) -> Self {
        let names = std::iter::once(commander)
            .chain(deck_cards)
            .map(|card| card.name.clone())
            .collect();
        Self {
            commander_identity: ColorIdentity::new(&commander.color_identity),
            names,
        }
    }

    pub fn from_analysis(analysis: &AnalyzeResult) -> Self {
        Self::new(&analysis.commander, analysis.cards.iter().map(|c| &c.card))
    }

    pub fn contains(&self, name: &str) -> bool {
        self.names.contains(name)
    }

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
        ColorIdentity::new(&card.color_identity).is_subset_of(&self.commander_identity)
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
