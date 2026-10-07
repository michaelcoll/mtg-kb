use std::cmp::Ordering;

use crate::model::Card;

/// Ordre des deux signaux de qualité qui départagent les Candidats à score
/// égal (ADR 0006) : le rang EDHREC d'abord, sauf quand la courbe de mana est
/// un Point faible, où la mana value passe devant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TieBreak {
    EdhrecRankFirst,
    ManaValueFirst,
}

/// Trie des Candidats par score décroissant, puis selon `tie_break` par rang
/// EDHREC croissant (absent en dernier) et mana value croissante, et enfin
/// par nom : l'ordre est total, donc déterministe.
pub fn sort_candidates<T>(
    items: &mut [T],
    score: impl Fn(&T) -> u32,
    card: impl Fn(&T) -> &Card,
    tie_break: TieBreak,
) {
    let by_rank = |a: &Card, b: &Card| {
        (a.edhrec_rank.is_none(), a.edhrec_rank).cmp(&(b.edhrec_rank.is_none(), b.edhrec_rank))
    };
    let by_mana_value = |a: &Card, b: &Card| {
        a.mana_value
            .unwrap_or(0.0)
            .total_cmp(&b.mana_value.unwrap_or(0.0))
    };
    items.sort_by(|a, b| {
        let (card_a, card_b) = (card(a), card(b));
        let signals = match tie_break {
            TieBreak::EdhrecRankFirst => {
                by_rank(card_a, card_b).then_with(|| by_mana_value(card_a, card_b))
            }
            TieBreak::ManaValueFirst => {
                by_mana_value(card_a, card_b).then_with(|| by_rank(card_a, card_b))
            }
        };
        score(b)
            .cmp(&score(a))
            .then(signals)
            .then_with(|| card_a.name.cmp(&card_b.name))
    });
}

pub fn sort_desc_by_score_then_name<T>(
    items: &mut [T],
    score: impl Fn(&T) -> f64,
    name: impl Fn(&T) -> &str,
) {
    items.sort_by(|a, b| {
        score(b)
            .partial_cmp(&score(a))
            .unwrap_or(Ordering::Equal)
            .then_with(|| name(a).cmp(name(b)))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_by_score_descending_then_name_ascending() {
        let mut items = vec![("Beta", 1.0), ("Alpha", 2.0), ("Gamma", 2.0)];
        sort_desc_by_score_then_name(&mut items, |i| i.1, |i| i.0);
        assert_eq!(
            items,
            vec![("Alpha", 2.0), ("Gamma", 2.0), ("Beta", 1.0)],
            "equal scores break ties by name ascending"
        );
    }
}
