use std::collections::BTreeMap;

use crate::model::{
    EnrichedAnalysis, ManaBase, ManaCurve, ReferencePrinting, Suggestion, Synergy, UnresolvedLine,
    Verdict,
};

pub mod validate;

/// Nom de fichier "slug" dérivé du nom du Commandant : minuscules,
/// caractères non alphanumériques réduits à des tirets simples.
pub fn slugify(name: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = true; // évite un tiret en tête
    for c in name.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
            last_was_dash = false;
        } else if !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }
    slug.trim_end_matches('-').to_string()
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn list_or_none(items: &[String]) -> String {
    if items.is_empty() {
        return "<p class=\"muted\">Aucun.</p>".to_string();
    }
    let lis: String = items
        .iter()
        .map(|i| format!("<li>{}</li>", escape_html(i)))
        .collect();
    format!("<ul>{lis}</ul>")
}

fn ordered_list_or_none(items: &[String]) -> String {
    if items.is_empty() {
        return "<p class=\"muted\">Aucune.</p>".to_string();
    }
    let lis: String = items
        .iter()
        .map(|i| format!("<li>{}</li>", escape_html(i)))
        .collect();
    format!("<ol>{lis}</ol>")
}

const SECTION_H2_STYLE: &str = "margin-top:0;border-top:none;padding-top:0;border-bottom:none";

fn render_head(commander_name: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="fr">
<head>
<meta charset="utf-8">
<title>Rapport d'analyse — {commander_name}</title>
<style>
  :root {{
    --bg: #0f1115; --panel: #171a21; --text: #e8eaed; --muted: #9aa0a6;
    --accent: #7c9cff; --ok: #3ecf8e; --error: #ff6b6b; --border: #2a2e37;
  }}
  * {{ box-sizing: border-box; }}
  body {{
    margin: 0; padding: 2rem; background: var(--bg); color: var(--text);
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    line-height: 1.5;
  }}
  main {{ max-width: 880px; margin: 0 auto; }}
  h1 {{ font-size: 1.75rem; margin-bottom: 0.25rem; }}
  h2 {{ font-size: 1.15rem; margin-top: 2.5rem; border-bottom: 1px solid var(--border); padding-bottom: 0.5rem; }}
  h3 {{ font-size: 0.85rem; margin: 1rem 0 0.35rem; color: var(--muted); text-transform: uppercase; letter-spacing: 0.03em; }}
  .muted {{ color: var(--muted); }}
  .badge {{ display: inline-block; padding: 0.2rem 0.6rem; border-radius: 999px; font-size: 0.85rem; font-weight: 600; }}
  .badge-ok {{ background: rgba(62,207,142,0.15); color: var(--ok); }}
  .badge-error {{ background: rgba(255,107,107,0.15); color: var(--error); }}
  section {{ background: var(--panel); border: 1px solid var(--border); border-radius: 12px; padding: 1.25rem 1.5rem; margin-top: 1rem; }}
  table {{ width: 100%; border-collapse: collapse; }}
  td, th {{ padding: 0.35rem 0.5rem; text-align: left; border-bottom: 1px solid var(--border); }}
  ul {{ margin: 0; padding-left: 1.25rem; }}
  li {{ margin-bottom: 0.4rem; }}
  .bar-row {{ display: flex; align-items: center; gap: 0.75rem; margin-bottom: 0.4rem; }}
  .bar-label {{ width: 2rem; text-align: right; color: var(--muted); }}
  .bar-track {{ flex: 1; background: var(--border); border-radius: 4px; height: 0.9rem; overflow: hidden; }}
  .bar-fill {{ background: var(--accent); height: 100%; }}
  .bar-count {{ width: 2rem; color: var(--muted); }}
  .stat {{ display: inline-block; margin-right: 2rem; }}
  .stat strong {{ display: block; font-size: 1.4rem; }}
  .commander-art {{ display: block; float: left; margin: 0 1rem 0.5rem 0; }}
  .commander-art img {{ display: block; width: 180px; border-radius: 12px; border: 1px solid var(--border); }}
  .suggestion-list {{ list-style: none; padding: 0; }}
  .suggestion-row {{ display: flex; align-items: flex-start; gap: 0.75rem; }}
  .suggestion-art {{ flex: none; display: block; }}
  .suggestion-art img {{ display: block; width: 60px; border-radius: 6px; border: 1px solid var(--border); }}
  .suggestion-body {{ flex: 1 1 auto; min-width: 0; }}
  .card-to-remove {{ display: flex; align-items: center; gap: 0.4rem; margin-top: 0.4rem; font-size: 0.85rem; }}
  .card-to-remove-art {{ flex: none; display: block; }}
  .card-to-remove-art img {{ display: block; width: 32px; border-radius: 4px; border: 1px solid var(--border); }}
  @media (max-width: 480px) {{
    .suggestion-art img {{ width: 44px; }}
    .card-to-remove-art img {{ width: 26px; }}
  }}
</style>
</head>
"#
    )
}

/// Portrait d'une Carte depuis l'endpoint officiel Scryfall, avec lien vers
/// la page de l'Impression de référence et repli sur le nom si l'image ne
/// charge pas (hors ligne) ou si aucune Impression de référence n'a de
/// `scryfallId`. `css_class` distingue la taille (portrait du Commandant en
/// en-tête, vignette dans une Suggestion).
fn render_card_art(name: &str, printing: Option<&ReferencePrinting>, css_class: &str) -> String {
    let Some(printing) = printing else {
        return String::new();
    };
    let scryfall_id = &printing.scryfall_id;
    let set_code = printing.set_code.to_lowercase();
    let number = &printing.number;
    format!(
        r#"<span class="{css_class}">
    <a href="https://scryfall.com/card/{set_code}/{number}" target="_blank" rel="noopener">
      <img src="https://api.scryfall.com/cards/{scryfall_id}?format=image&amp;version=normal" alt="{name}" onerror="this.hidden=true;this.nextElementSibling.hidden=false;">
    </a>
    <span class="{css_class}-fallback muted" hidden>{name}</span>
  </span>
  "#
    )
}

fn render_header_section(
    commander_name: &str,
    card_count: u32,
    verdict_badge: &str,
    printing: Option<&ReferencePrinting>,
) -> String {
    let art = render_card_art(commander_name, printing, "commander-art");
    format!(
        r#"{art}<h1>{commander_name}</h1>
  <p class="muted">{card_count} cartes {verdict_badge}</p>
"#
    )
}

fn render_verdict_section(verdict: &Verdict) -> String {
    let summary = escape_html(&verdict.summary);
    let strengths = list_or_none(&verdict.strengths);
    let weaknesses = list_or_none(&verdict.weaknesses);
    let priorities = ordered_list_or_none(&verdict.priorities);
    format!(
        r#"  <section>
    <h2 style="{SECTION_H2_STYLE}">Verdict</h2>
    <p>{summary}</p>
    <h3>Points forts</h3>
    {strengths}
    <h3>Faiblesses</h3>
    {weaknesses}
    <h3>Priorités</h3>
    {priorities}
  </section>
"#
    )
}

fn render_mana_curve_section(curve: &ManaCurve) -> String {
    let max_bucket_count = curve.buckets.iter().map(|b| b.count).max().unwrap_or(1);
    let curve_bars: String = curve
        .buckets
        .iter()
        .map(|b| {
            let width = (b.count as f64 / max_bucket_count.max(1) as f64 * 100.0).round();
            let label = if b.mana_value >= 7 {
                "7+".to_string()
            } else {
                b.mana_value.to_string()
            };
            format!(
                "<div class=\"bar-row\"><span class=\"bar-label\">{label}</span>\
                 <div class=\"bar-track\"><div class=\"bar-fill\" style=\"width:{width}%\"></div></div>\
                 <span class=\"bar-count\">{}</span></div>",
                b.count
            )
        })
        .collect();
    let avg_mv = curve.average_mana_value;

    format!(
        r#"  <section>
    <h2 style="{SECTION_H2_STYLE}">Courbe de mana</h2>
    <p class="stat"><strong>{avg_mv}</strong><span class="muted">mana value moyenne (hors terrains)</span></p>
    {curve_bars}
  </section>
"#
    )
}

fn render_mana_base_section(mana_base: &ManaBase) -> String {
    let sources_rows: String = mana_base
        .sources_by_color
        .iter()
        .map(|(color, count)| {
            let demand = mana_base.symbols_by_color.get(color).unwrap_or(&0);
            format!("<tr><td>{color}</td><td>{count}</td><td>{demand}</td></tr>")
        })
        .collect();
    let land_count = mana_base.land_count;

    format!(
        r#"  <section>
    <h2 style="{SECTION_H2_STYLE}">Base de mana</h2>
    <p class="stat"><strong>{land_count}</strong><span class="muted">terrains</span></p>
    <table>
      <thead><tr><th>Couleur</th><th>Sources</th><th>Symboles demandés</th></tr></thead>
      <tbody>{sources_rows}</tbody>
    </table>
  </section>
"#
    )
}

fn render_roles_section(role_counts: &BTreeMap<String, u32>) -> String {
    let role_rows: String = role_counts
        .iter()
        .map(|(role, count)| format!("<tr><td>{}</td><td>{count}</td></tr>", escape_html(role)))
        .collect();

    format!(
        r#"  <section>
    <h2 style="{SECTION_H2_STYLE}">Rôles</h2>
    <table>
      <thead><tr><th>Rôle</th><th>Cartes</th></tr></thead>
      <tbody>{role_rows}</tbody>
    </table>
  </section>
"#
    )
}

fn render_weaknesses_section(weaknesses: &[String]) -> String {
    let weaknesses = list_or_none(weaknesses);
    format!(
        r#"  <section>
    <h2 style="{SECTION_H2_STYLE}">Points faibles</h2>
    {weaknesses}
  </section>
"#
    )
}

fn render_synergies_section(synergies: &[Synergy]) -> String {
    let synergy_items: String = synergies
        .iter()
        .map(|s| {
            format!(
                "<li><strong>{}</strong> — {}</li>",
                escape_html(&s.theme),
                escape_html(&s.cards.join(", "))
            )
        })
        .collect();

    format!(
        r#"  <section>
    <h2 style="{SECTION_H2_STYLE}">Synergies</h2>
    <ul>{synergy_items}</ul>
  </section>
"#
    )
}

/// Chaque Suggestion en ligne : vignette de l'Impression de référence à
/// gauche (même mécanisme que le portrait du Commandant), nom et
/// justification à droite. `printings[i]` correspond à `suggestions[i]`.
/// Bloc « Carte à retirer » (voir CONTEXT.md) : plus petit que la
/// Suggestion, avec sa propre vignette si une Impression de référence est
/// connue. Absent du rendu si la Suggestion n'en propose pas.
fn render_card_to_remove(
    card_to_remove: Option<&String>,
    printing: Option<&ReferencePrinting>,
) -> String {
    let Some(card_to_remove) = card_to_remove else {
        return String::new();
    };
    let name = escape_html(card_to_remove);
    let art = render_card_art(&name, printing, "card-to-remove-art");
    format!(
        "<span class=\"card-to-remove\"><span class=\"muted\">Remplace :</span> {art}<span class=\"card-to-remove-name\">{name}</span></span>"
    )
}

fn render_suggestions_section(
    suggestions: &[Suggestion],
    printings: &[Option<ReferencePrinting>],
    card_to_remove_printings: &[Option<ReferencePrinting>],
) -> String {
    debug_assert_eq!(
        suggestions.len(),
        printings.len(),
        "suggestions et printings doivent être alignés positionnellement"
    );
    debug_assert_eq!(
        suggestions.len(),
        card_to_remove_printings.len(),
        "suggestions et card_to_remove_printings doivent être alignés positionnellement"
    );
    let suggestion_items: String = suggestions
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let name = escape_html(&s.card_name);
            let printing = printings.get(i).and_then(|p| p.as_ref());
            let art = render_card_art(&name, printing, "suggestion-art");
            let card_to_remove_printing = card_to_remove_printings.get(i).and_then(|p| p.as_ref());
            let card_to_remove =
                render_card_to_remove(s.card_to_remove.as_ref(), card_to_remove_printing);
            format!(
                "<li class=\"suggestion-row\">{art}<span class=\"suggestion-body\"><strong>{name}</strong><p>{}</p>{card_to_remove}</span></li>",
                escape_html(&s.justification)
            )
        })
        .collect();

    format!(
        r#"  <section>
    <h2 style="{SECTION_H2_STYLE}">Suggestions</h2>
    <ul class="suggestion-list">{suggestion_items}</ul>
  </section>
"#
    )
}

fn render_unresolved_section(unresolved: &[UnresolvedLine]) -> String {
    let unresolved_items: String = unresolved
        .iter()
        .map(|u| format!("<li>{} x{}</li>", escape_html(&u.name), u.quantity))
        .collect();
    let unresolved = if unresolved_items.is_empty() {
        "<p class=\"muted\">Aucune.</p>".to_string()
    } else {
        format!("<ul>{unresolved_items}</ul>")
    };

    format!(
        r#"  <section>
    <h2 style="{SECTION_H2_STYLE}">Cartes non résolues</h2>
    {unresolved}
  </section>
"#
    )
}

/// Retire les espaces de fin et ajoute un unique saut de ligne, pour
/// recoller des sections rendues indépendamment dans un gabarit commun.
fn trimmed_with_newline(s: String) -> String {
    format!("{}\n", s.trim_end())
}

/// Rendu HTML autonome (MVP) du Rapport d'analyse à partir du JSON de
/// `kb analyze` enrichi par Claude (verdict, Suggestions retenues).
///
/// Chaque section est rendue par sa propre fonction (issue #21) pour que les
/// évolutions futures (Verdict structuré, images Scryfall, ...) touchent une
/// seule section sans toucher aux autres.
pub fn render(
    enriched: &EnrichedAnalysis,
    commander_printing: Option<&ReferencePrinting>,
    suggestion_printings: &[Option<ReferencePrinting>],
    card_to_remove_printings: &[Option<ReferencePrinting>],
) -> String {
    let a = &enriched.analysis;

    let verdict_badge = if a.construction_errors.is_empty() {
        "<span class=\"badge badge-ok\">Deck valide</span>"
    } else {
        "<span class=\"badge badge-error\">Erreurs de construction</span>"
    };
    let commander_name = escape_html(&a.commander.name);

    let head = render_head(&commander_name);
    let header = trimmed_with_newline(render_header_section(
        &commander_name,
        a.card_count,
        verdict_badge,
        commander_printing,
    ));
    let verdict = trimmed_with_newline(render_verdict_section(&enriched.verdict));
    let mana_curve = trimmed_with_newline(render_mana_curve_section(&a.mana_curve));
    let mana_base = trimmed_with_newline(render_mana_base_section(&a.mana_base));
    let roles = trimmed_with_newline(render_roles_section(&a.role_counts));
    let weaknesses = trimmed_with_newline(render_weaknesses_section(&a.weaknesses));
    let synergies = trimmed_with_newline(render_synergies_section(&a.synergies));
    let suggestions = trimmed_with_newline(render_suggestions_section(
        &enriched.suggestions,
        suggestion_printings,
        card_to_remove_printings,
    ));
    let unresolved = render_unresolved_section(&a.unresolved);

    format!(
        "{head}<body>\n<main>\n  {header}\n{verdict}\n{mana_curve}\n{mana_base}\n{roles}\n{weaknesses}\n{synergies}\n{suggestions}\n{unresolved}</main>\n</body>\n</html>\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    use std::collections::BTreeMap;

    pub(super) fn sample() -> EnrichedAnalysis {
        EnrichedAnalysis {
            analysis: AnalyzeResult {
                commander: Card {
                    name: "Atraxa, Praetors' Voice".to_string(),
                    mana_cost: Some("{G}{W}{U}{B}".to_string()),
                    mana_value: Some(4.0),
                    type_line: None,
                    types: vec!["Creature".to_string()],
                    subtypes: vec![],
                    supertypes: vec![],
                    oracle_text: None,
                    color_identity: vec![
                        "B".to_string(),
                        "G".to_string(),
                        "U".to_string(),
                        "W".to_string(),
                    ],
                    colors: vec![],
                    keywords: vec![],
                    power: None,
                    toughness: None,
                    loyalty: None,
                },
                cards: vec![],
                unresolved: vec![UnresolvedLine {
                    quantity: 1,
                    name: "<script>alert(1)</script>".to_string(),
                }],
                card_count: 100,
                construction_errors: vec![],
                mana_curve: ManaCurve {
                    buckets: vec![ManaCurveBucket {
                        mana_value: 2,
                        count: 5,
                    }],
                    average_mana_value: 2.5,
                },
                mana_base: ManaBase {
                    land_count: 37,
                    sources_by_color: BTreeMap::new(),
                    symbols_by_color: BTreeMap::new(),
                },
                role_counts: BTreeMap::new(),
                weaknesses: vec!["Ramp sous-représenté : 3 cartes (< 10)".to_string()],
                synergies: vec![Synergy {
                    theme: "tokens".to_string(),
                    cards: vec!["Krenko, Mob Boss".to_string()],
                }],
                candidates: vec![],
            },
            verdict: Verdict {
                summary: "Solide, manque de ramp".to_string(),
                strengths: vec!["Base de mana solide".to_string()],
                weaknesses: vec!["Manque de ramp".to_string()],
                priorities: vec!["Ajouter 2-3 sources de ramp".to_string()],
            },
            suggestions: vec![Suggestion {
                card_name: "Rampant Growth".to_string(),
                justification: "Comble le manque de ramp".to_string(),
                card_to_remove: None,
            }],
        }
    }

    #[test]
    fn slugify_normalizes_commander_name() {
        assert_eq!(slugify("Atraxa, Praetors' Voice"), "atraxa-praetors-voice");
    }

    #[test]
    fn renders_all_sections() {
        let html = render(&sample(), None, &[None], &[None]);
        assert!(html.contains("Atraxa, Praetors&#39; Voice"));
        assert!(html.contains("Solide, manque de ramp"));
        assert!(html.contains("Rampant Growth"));
        assert!(html.contains("Ramp sous-représenté"));
        assert!(html.contains("tokens"));
        assert!(html.contains("Krenko, Mob Boss"));
    }

    #[test]
    fn escapes_untrusted_content() {
        let html = render(&sample(), None, &[None], &[None]);
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(html.contains("&lt;script&gt;"));
    }

    /// Non-régression du rendu (issue #21) : le HTML produit pour une
    /// analyse de référence ne doit pas changer d'un octet en dehors des
    /// évolutions volontaires (Verdict structuré, images Scryfall, ...).
    #[test]
    fn render_output_is_stable_across_the_section_split() {
        let html = render(&sample(), None, &[None], &[None]);
        assert_eq!(html, include_str!("golden_sample.html"));
    }

    #[test]
    fn renders_structured_verdict_sections() {
        let html = render(&sample(), None, &[None]);
        assert!(html.contains("Points forts"));
        assert!(html.contains("Base de mana solide"));
        assert!(html.contains("Faiblesses"));
        assert!(html.contains("Manque de ramp"));
        assert!(html.contains("Priorités"));
        assert!(html.contains("Ajouter 2-3 sources de ramp"));
        assert!(html.contains("<ol>"));
    }

    #[test]
    fn escapes_untrusted_content_in_verdict_lists() {
        let mut enriched = sample();
        enriched.verdict.strengths = vec!["<script>alert(1)</script>".to_string()];
        let html = render(&enriched, None, &[None]);
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn renders_aucune_for_empty_verdict_priorities() {
        let mut enriched = sample();
        enriched.verdict.priorities = vec![];
        let html = render(&enriched, None, &[None]);
        assert!(html.contains("Aucune."));
    }

    #[test]
    fn renders_commander_image_with_scryfall_link_when_printing_is_known() {
        let printing = ReferencePrinting {
            scryfall_id: "abc-123".to_string(),
            set_code: "M15".to_string(),
            number: "4".to_string(),
        };
        let html = render(&sample(), Some(&printing), &[None], &[None]);
        assert!(
            html.contains("https://api.scryfall.com/cards/abc-123?format=image&amp;version=normal")
        );
        assert!(html.contains("https://scryfall.com/card/m15/4"));
    }

    #[test]
    fn falls_back_to_name_when_no_reference_printing() {
        let html = render(&sample(), None, &[None], &[None]);
        assert!(!html.contains("scryfall.com"));
        assert!(html.contains("<h1>Atraxa, Praetors&#39; Voice</h1>"));
    }

    #[test]
    fn renders_suggestion_image_with_scryfall_link_when_printing_is_known() {
        let printing = ReferencePrinting {
            scryfall_id: "rg-123".to_string(),
            set_code: "M19".to_string(),
            number: "191".to_string(),
        };
        let html = render(&sample(), None, &[Some(printing)], &[None]);
        assert!(
            html.contains("https://api.scryfall.com/cards/rg-123?format=image&amp;version=normal")
        );
        assert!(html.contains("https://scryfall.com/card/m19/191"));
        assert!(html.contains("Rampant Growth"));
        assert!(html.contains("Comble le manque de ramp"));
    }

    #[test]
    fn falls_back_to_name_for_a_suggestion_without_reference_printing() {
        let html = render(&sample(), None, &[None], &[None]);
        assert!(!html.contains("scryfall.com"));
        assert!(html.contains("Rampant Growth"));
    }

    #[test]
    fn renders_unchanged_without_a_card_to_remove() {
        let html = render(&sample(), None, &[None], &[None]);
        assert!(!html.contains("class=\"card-to-remove\""));
    }

    #[test]
    fn renders_card_to_remove_smaller_with_its_own_image() {
        let mut enriched = sample();
        enriched.suggestions[0].card_to_remove = Some("Llanowar Elves".to_string());
        let printing = ReferencePrinting {
            scryfall_id: "elves-123".to_string(),
            set_code: "M19".to_string(),
            number: "183".to_string(),
        };
        let html = render(&enriched, None, &[None], &[Some(printing)]);
        assert!(html.contains("class=\"card-to-remove\""));
        assert!(html.contains("card-to-remove-art"));
        assert!(html.contains("Llanowar Elves"));
        assert!(
            html.contains(
                "https://api.scryfall.com/cards/elves-123?format=image&amp;version=normal"
            )
        );
    }

    #[test]
    fn escapes_untrusted_content_in_card_to_remove_name() {
        let mut enriched = sample();
        enriched.suggestions[0].card_to_remove = Some("<script>alert(1)</script>".to_string());
        let html = render(&enriched, None, &[None], &[None]);
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(html.contains("&lt;script&gt;"));
    }
}
