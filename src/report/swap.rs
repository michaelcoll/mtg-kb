//! Simulation d'un échange (ajout de la Suggestion, retrait de la Carte à
//! retirer) sur les comptes de Rôles à seuil. Un avertissement n'est qu'un
//! signal : il ne bloque jamais le Rapport (ADR 0006).

use crate::model::AnalyzeResult;

/// Un Rôle à seuil que l'échange laisse sous son minimum, en le faisant
/// baisser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwapWarning {
    pub role: String,
    pub count_after: u32,
    pub minimum: u32,
}

/// Avertit pour chaque Rôle à seuil qui baisse et finit sous son minimum : un
/// Rôle déjà sous son seuil n'avertit donc que s'il baisse encore.
pub(super) fn swap_warnings(
    analysis: &AnalyzeResult,
    suggestion_roles: &[String],
    card_to_remove: &str,
) -> Vec<SwapWarning> {
    let removed_roles: &[String] = analysis
        .cards
        .iter()
        .find(|c| c.card.name == card_to_remove)
        .map_or(&[], |c| c.roles.as_slice());

    analysis
        .thresholds
        .role_minimums()
        .into_iter()
        .filter_map(|(role, minimum)| {
            let before = *analysis.role_counts.get(role).unwrap_or(&0);
            let added = u32::from(suggestion_roles.iter().any(|r| r == role));
            let removed = u32::from(removed_roles.iter().any(|r| r == role));
            let count_after = (before + added).saturating_sub(removed);
            (count_after < before && count_after < minimum).then(|| SwapWarning {
                role: role.to_string(),
                count_after,
                minimum,
            })
        })
        .collect()
}
