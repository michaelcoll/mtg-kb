use std::collections::BTreeMap;

use crate::model::{EnrichedAnalysis, ManaBase, ManaCurve, Suggestion, Synergy, UnresolvedLine};

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
</style>
</head>
"#
    )
}

fn render_header_section(commander_name: &str, card_count: u32, verdict_badge: &str) -> String {
    format!(
        r#"<h1>{commander_name}</h1>
  <p class="muted">{card_count} cartes {verdict_badge}</p>
"#
    )
}

fn render_verdict_section(verdict: &str) -> String {
    format!(
        r#"  <section>
    <h2 style="{SECTION_H2_STYLE}">Verdict</h2>
    <p>{verdict}</p>
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

fn render_suggestions_section(suggestions: &[Suggestion]) -> String {
    let suggestion_items: String = suggestions
        .iter()
        .map(|s| {
            format!(
                "<li><strong>{}</strong><p>{}</p></li>",
                escape_html(&s.card_name),
                escape_html(&s.justification)
            )
        })
        .collect();

    format!(
        r#"  <section>
    <h2 style="{SECTION_H2_STYLE}">Suggestions</h2>
    <ul>{suggestion_items}</ul>
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

/// Rendu HTML autonome (MVP) du Rapport d'analyse à partir du JSON de
/// `kb analyze` enrichi par Claude (verdict, Suggestions retenues).
///
/// Chaque section est rendue par sa propre fonction (issue #21) pour que les
/// évolutions futures (Verdict structuré, images Scryfall, ...) touchent une
/// seule section sans toucher aux autres.
/// Retire les espaces de fin et ajoute un unique saut de ligne, pour
/// recoller des sections rendues indépendamment dans un gabarit commun.
fn trimmed_with_newline(s: String) -> String {
    format!("{}\n", s.trim_end())
}

pub fn render(enriched: &EnrichedAnalysis) -> String {
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
    ));
    let verdict = trimmed_with_newline(render_verdict_section(&escape_html(&enriched.verdict)));
    let mana_curve = trimmed_with_newline(render_mana_curve_section(&a.mana_curve));
    let mana_base = trimmed_with_newline(render_mana_base_section(&a.mana_base));
    let roles = trimmed_with_newline(render_roles_section(&a.role_counts));
    let weaknesses = trimmed_with_newline(render_weaknesses_section(&a.weaknesses));
    let synergies = trimmed_with_newline(render_synergies_section(&a.synergies));
    let suggestions = trimmed_with_newline(render_suggestions_section(&enriched.suggestions));
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
            verdict: "Solide, manque de ramp".to_string(),
            suggestions: vec![Suggestion {
                card_name: "Rampant Growth".to_string(),
                justification: "Comble le manque de ramp".to_string(),
            }],
        }
    }

    #[test]
    fn slugify_normalizes_commander_name() {
        assert_eq!(slugify("Atraxa, Praetors' Voice"), "atraxa-praetors-voice");
    }

    #[test]
    fn renders_all_sections() {
        let html = render(&sample());
        assert!(html.contains("Atraxa, Praetors&#39; Voice"));
        assert!(html.contains("Solide, manque de ramp"));
        assert!(html.contains("Rampant Growth"));
        assert!(html.contains("Ramp sous-représenté"));
        assert!(html.contains("tokens"));
        assert!(html.contains("Krenko, Mob Boss"));
    }

    #[test]
    fn escapes_untrusted_content() {
        let html = render(&sample());
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(html.contains("&lt;script&gt;"));
    }

    /// Non-régression du découpage par section (issue #21) : le HTML produit
    /// pour une analyse de référence ne doit pas changer d'un octet.
    #[test]
    fn render_output_is_stable_across_the_section_split() {
        let html = render(&sample());
        assert_eq!(html, include_str!("golden_sample.html"));
    }
}
