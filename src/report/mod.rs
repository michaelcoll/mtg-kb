use std::collections::{BTreeMap, HashSet};

use crate::model::{
    EdhrecRecommendation, ManaBase, ManaCurve, RecommanderRecommendation, ReferencePrinting,
    SourceError, Synergy, UnresolvedLine, Verdict,
};

mod origins;
mod prepare;
mod validate;

pub use origins::Origin;
pub use prepare::{CardPrintings, PrintingLookup, ReportModel, ValidatedSuggestion, prepare};

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
  .badge-origin {{ background: rgba(124,156,255,0.15); color: var(--accent); font-size: 0.7rem; padding: 0.1rem 0.5rem; margin-left: 0.35rem; text-transform: uppercase; letter-spacing: 0.03em; }}
  .warning {{ border-color: var(--error); }}
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
  a {{ color: var(--accent); }}
  .hover-target {{ cursor: pointer; }}
  .hover-preview {{ position: fixed; z-index: 1000; display: flex; gap: 0.5rem; pointer-events: none; }}
  .hover-preview[hidden] {{ display: none; }}
  .hover-preview-face {{ display: block; width: 240px; border-radius: 12px; border: 1px solid var(--border); box-shadow: 0 8px 24px rgba(0,0,0,0.55); }}
</style>
</head>
"#
    )
}

/// Vignette agrandie partagée au survol/focus des `.hover-target`, lisant
/// `data-hover-img` et, pour les Cartes à deux Faces, `data-hover-img-back`.
fn render_hover_preview_markup() -> String {
    r#"  <div id="hover-preview" class="hover-preview" hidden>
    <img class="hover-preview-face hover-preview-face-front" alt="">
    <img class="hover-preview-face hover-preview-face-back" alt="" hidden>
  </div>
  <script>
  (function () {
    var preview = document.getElementById('hover-preview');
    var front = preview.querySelector('.hover-preview-face-front');
    var back = preview.querySelector('.hover-preview-face-back');
    var margin = 8;

    function hide() {
      preview.hidden = true;
    }

    function position(target) {
      var rect = target.getBoundingClientRect();
      preview.style.left = '0px';
      preview.style.top = '0px';
      var pw = preview.offsetWidth;
      var ph = preview.offsetHeight;
      var left = rect.right + margin;
      if (left + pw > window.innerWidth - margin) {
        left = rect.left - margin - pw;
      }
      if (left < margin) {
        left = Math.min(window.innerWidth - pw - margin, Math.max(margin, rect.left));
      }
      var top = rect.top;
      if (top + ph > window.innerHeight - margin) {
        top = window.innerHeight - margin - ph;
      }
      if (top < margin) {
        top = margin;
      }
      preview.style.left = left + 'px';
      preview.style.top = top + 'px';
    }

    var currentTarget = null;

    function show(target) {
      var src = target.getAttribute('data-hover-img');
      if (!src) {
        return;
      }
      currentTarget = target;
      var backSrc = target.getAttribute('data-hover-img-back');
      front.hidden = false;
      if (front.src !== src) {
        front.src = src;
      }
      if (backSrc) {
        back.hidden = false;
        if (back.src !== backSrc) {
          back.src = backSrc;
        }
      } else {
        back.removeAttribute('src');
        back.hidden = true;
      }
      preview.hidden = false;
      position(target);
    }

    // Les vignettes statiques gèrent leur propre repli via onerror.
    function degradeIfNoStaticImage() {
      if (currentTarget && !currentTarget.querySelector('img')) {
        var span = document.createElement('span');
        span.className = 'muted';
        span.textContent = currentTarget.textContent;
        currentTarget.replaceWith(span);
      }
      currentTarget = null;
    }

    function handleFaceError(face, otherFace) {
      face.hidden = true;
      if (otherFace.hidden) {
        hide();
        degradeIfNoStaticImage();
      }
    }

    front.addEventListener('error', function () {
      handleFaceError(front, back);
    });
    back.addEventListener('error', function () {
      handleFaceError(back, front);
    });
    document.querySelectorAll('.hover-target').forEach(function (el) {
      el.addEventListener('mouseenter', function () {
        show(el);
      });
      el.addEventListener('mouseleave', hide);
      el.addEventListener('focus', function () {
        show(el);
      });
      el.addEventListener('blur', hide);
    });
  })();
  </script>
"#
    .to_string()
}

fn face_image_url(scryfall_id: &str, back: bool) -> String {
    if back {
        format!(
            "https://api.scryfall.com/cards/{scryfall_id}?format=image&amp;version=normal&amp;face=back"
        )
    } else {
        format!("https://api.scryfall.com/cards/{scryfall_id}?format=image&amp;version=normal")
    }
}

fn hover_back_attr(printing: &ReferencePrinting) -> String {
    if printing.is_two_faced {
        format!(
            r#" data-hover-img-back="{}""#,
            face_image_url(&printing.scryfall_id, true)
        )
    } else {
        String::new()
    }
}

/// Vignette Scryfall (face avant) liée à l'Impression de référence, avec
/// repli sur le nom si l'image ne charge pas. `name` est brut : échappé ici.
fn render_card_art(name: &str, printing: Option<&ReferencePrinting>, css_class: &str) -> String {
    let Some(printing) = printing else {
        return String::new();
    };
    let name = escape_html(name);
    let set_code = printing.set_code.to_lowercase();
    let number = &printing.number;
    let image_url = face_image_url(&printing.scryfall_id, false);
    let back_attr = hover_back_attr(printing);
    format!(
        r#"<span class="{css_class}">
    <a href="https://scryfall.com/card/{set_code}/{number}" target="_blank" rel="noopener" class="hover-target" data-hover-img="{image_url}"{back_attr}>
      <img src="{image_url}" alt="{name}" onerror="this.hidden=true;this.nextElementSibling.hidden=false;">
    </a>
    <span class="{css_class}-fallback muted" hidden>{name}</span>
  </span>
  "#
    )
}

/// Comme `render_card_art`, mais sans image statique : seul le nom est affiché.
fn hover_card_link(name: &str, printing: Option<&ReferencePrinting>) -> String {
    let escaped = escape_html(name);
    let Some(printing) = printing else {
        return escaped;
    };
    let set_code = printing.set_code.to_lowercase();
    let number = &printing.number;
    let image_url = face_image_url(&printing.scryfall_id, false);
    let back_attr = hover_back_attr(printing);
    format!(
        r#"<a href="https://scryfall.com/card/{set_code}/{number}" target="_blank" rel="noopener" class="hover-target" data-hover-img="{image_url}"{back_attr}>{escaped}</a>"#
    )
}

fn render_header_section(
    commander_name: &str,
    card_count: u32,
    verdict_badge: &str,
    printing: Option<&ReferencePrinting>,
) -> String {
    let art = render_card_art(commander_name, printing, "commander-art");
    let commander_name = escape_html(commander_name);
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

fn render_synergies_section(synergies: &[Synergy], card_printings: &CardPrintings) -> String {
    let synergy_items: String = synergies
        .iter()
        .map(|s| {
            let cards: String = s
                .cards
                .iter()
                .map(|c| hover_card_link(c, card_printings.get(c)))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "<li><strong>{}</strong> — {}</li>",
                escape_html(&s.theme),
                cards
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

fn render_card_to_remove(
    card_to_remove: Option<&str>,
    printing: Option<&ReferencePrinting>,
) -> String {
    let Some(card_to_remove) = card_to_remove else {
        return String::new();
    };
    let art = render_card_art(card_to_remove, printing, "card-to-remove-art");
    let name = escape_html(card_to_remove);
    format!(
        "<span class=\"card-to-remove\"><span class=\"muted\">Remplace :</span> {art}<span class=\"card-to-remove-name\">{name}</span></span>"
    )
}

fn render_origin_badges(origins: &[Origin]) -> String {
    origins
        .iter()
        .map(|o| format!("<span class=\"badge badge-origin\">{}</span>", o.label()))
        .collect()
}

fn render_suggestions_section(suggestions: &[ValidatedSuggestion]) -> String {
    let suggestion_items: String = suggestions
        .iter()
        .map(|s| {
            let art = render_card_art(&s.card_name, s.printing.as_ref(), "suggestion-art");
            let name = escape_html(&s.card_name);
            let badges = render_origin_badges(&s.origins);
            let card_to_remove = render_card_to_remove(
                s.card_to_remove.as_deref(),
                s.card_to_remove_printing.as_ref(),
            );
            format!(
                "<li class=\"suggestion-row\">{art}<span class=\"suggestion-body\"><strong>{name}</strong>{badges}<p>{}</p>{card_to_remove}</span></li>",
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

fn render_source_errors_section(source_errors: &[SourceError]) -> String {
    if source_errors.is_empty() {
        return String::new();
    }
    let items: String = source_errors
        .iter()
        .map(|e| {
            format!(
                "<li><strong>{}</strong> — {}</li>",
                escape_html(&e.source),
                escape_html(&e.message)
            )
        })
        .collect();
    format!(
        r#"  <section class="warning">
    <h2 style="{SECTION_H2_STYLE}">Avertissement</h2>
    <p class="muted">Une ou plusieurs Sources externes ont échoué ; l'analyse reste complète, mais peut manquer des Recommandations externes.</p>
    <ul>{items}</ul>
  </section>
"#
    )
}

/// Exigé par les conditions d'utilisation de Recommander.
fn render_credits_section() -> String {
    format!(
        r#"  <section>
    <h2 style="{SECTION_H2_STYLE}">Crédits</h2>
    <p class="muted">Recommandations externes fournies par <a href="https://edhrec.com" target="_blank" rel="noopener">EDHREC</a> et <a href="https://recommander.cards" target="_blank" rel="noopener">Recommander</a>.</p>
  </section>
"#
    )
}

fn render_unused_recommendations_list<T>(
    items: &[T],
    suggestion_names: &HashSet<&str>,
    card_printings: &CardPrintings,
    card_name: impl Fn(&T) -> &str,
    label: impl Fn(&T, &str) -> String,
) -> String {
    let list_items: String = items
        .iter()
        .filter(|r| !suggestion_names.contains(card_name(r)))
        .map(|r| {
            let name = card_name(r);
            let name_html = hover_card_link(name, card_printings.get(name));
            format!("<li>{}</li>", label(r, &name_html))
        })
        .collect();
    if list_items.is_empty() {
        "<p class=\"muted\">Aucune.</p>".to_string()
    } else {
        format!("<ul>{list_items}</ul>")
    }
}

fn render_external_appendix_section(
    edhrec_recommendations: &[EdhrecRecommendation],
    recommander_recommendations: &[RecommanderRecommendation],
    suggestion_names: &HashSet<&str>,
    card_printings: &CardPrintings,
) -> String {
    let edhrec_html = render_unused_recommendations_list(
        edhrec_recommendations,
        suggestion_names,
        card_printings,
        |r| r.card.name.as_str(),
        |r, name_html| format!("{name_html} (synergie {:.2})", r.synergy),
    );
    let recommander_html = render_unused_recommendations_list(
        recommander_recommendations,
        suggestion_names,
        card_printings,
        |r| r.card.name.as_str(),
        |r, name_html| format!("{name_html} (score {:.2})", r.score),
    );

    format!(
        r#"  <section>
    <h2 style="{SECTION_H2_STYLE}">Recommandations externes non retenues</h2>
    <h3>EDHREC</h3>
    {edhrec_html}
    <h3>Recommander</h3>
    {recommander_html}
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

fn trimmed_with_newline(s: String) -> String {
    format!("{}\n", s.trim_end())
}

pub fn render(model: &ReportModel) -> String {
    let a = &model.analysis;
    let card_printings = &model.card_printings;

    let verdict_badge = if a.construction_errors.is_empty() {
        "<span class=\"badge badge-ok\">Deck valide</span>"
    } else {
        "<span class=\"badge badge-error\">Erreurs de construction</span>"
    };
    let head = render_head(&escape_html(&a.commander.name));
    let header = trimmed_with_newline(render_header_section(
        &a.commander.name,
        a.card_count,
        verdict_badge,
        model.commander_printing.as_ref(),
    ));
    let verdict = trimmed_with_newline(render_verdict_section(&model.verdict));
    let source_errors = render_source_errors_section(&a.source_errors);
    let source_errors = if source_errors.is_empty() {
        String::new()
    } else {
        trimmed_with_newline(source_errors)
    };
    let mana_curve = trimmed_with_newline(render_mana_curve_section(&a.mana_curve));
    let mana_base = trimmed_with_newline(render_mana_base_section(&a.mana_base));
    let roles = trimmed_with_newline(render_roles_section(&a.role_counts));
    let weaknesses = trimmed_with_newline(render_weaknesses_section(&a.weaknesses));
    let synergies = trimmed_with_newline(render_synergies_section(&a.synergies, card_printings));
    let suggestions = trimmed_with_newline(render_suggestions_section(&model.suggestions));
    let suggestion_names: HashSet<&str> = model
        .suggestions
        .iter()
        .map(|s| s.card_name.as_str())
        .collect();
    let appendix = trimmed_with_newline(render_external_appendix_section(
        &a.edhrec_recommendations,
        &a.recommander_recommendations,
        &suggestion_names,
        card_printings,
    ));
    let credits = trimmed_with_newline(render_credits_section());
    let unresolved = render_unresolved_section(&a.unresolved);
    let hover_preview = render_hover_preview_markup();

    format!(
        "{head}<body>\n<main>\n  {header}\n{verdict}\n{source_errors}{mana_curve}\n{mana_base}\n{roles}\n{weaknesses}\n{synergies}\n{suggestions}\n{appendix}\n{credits}\n{unresolved}</main>\n{hover_preview}</body>\n</html>\n"
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
                commander: Card::named("Atraxa, Praetors' Voice", &["B", "G", "U", "W"]),
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
                edhrec_recommendations: vec![],
                edhrec_unresolved_names: vec![],
                recommander_recommendations: vec![],
                recommander_unresolved_names: vec![],
                source_errors: vec![],
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

    /// Modèle sans Impression de référence ni Origine, Suggestions reprises
    /// telles quelles.
    fn model(enriched: EnrichedAnalysis) -> ReportModel {
        ReportModel {
            suggestions: enriched
                .suggestions
                .into_iter()
                .map(|s| ValidatedSuggestion {
                    card_name: s.card_name,
                    justification: s.justification,
                    card_to_remove: s.card_to_remove,
                    printing: None,
                    card_to_remove_printing: None,
                    origins: vec![],
                })
                .collect(),
            analysis: enriched.analysis,
            verdict: enriched.verdict,
            commander_printing: None,
            card_printings: CardPrintings::new(),
        }
    }

    fn printing(scryfall_id: &str, set_code: &str, number: &str) -> ReferencePrinting {
        ReferencePrinting {
            scryfall_id: scryfall_id.to_string(),
            set_code: set_code.to_string(),
            number: number.to_string(),
            is_two_faced: false,
        }
    }

    #[test]
    fn renders_all_sections() {
        let html = render(&model(sample()));
        assert!(html.contains("Atraxa, Praetors&#39; Voice"));
        assert!(html.contains("Solide, manque de ramp"));
        assert!(html.contains("Rampant Growth"));
        assert!(html.contains("Ramp sous-représenté"));
        assert!(html.contains("tokens"));
        assert!(html.contains("Krenko, Mob Boss"));
    }

    #[test]
    fn escapes_untrusted_content() {
        let html = render(&model(sample()));
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn render_output_is_stable_across_the_section_split() {
        let html = render(&model(sample()));
        assert_eq!(html, include_str!("golden_sample.html"));
    }

    #[test]
    fn renders_structured_verdict_sections() {
        let html = render(&model(sample()));
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
        let html = render(&model(enriched));
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn renders_aucune_for_empty_verdict_priorities() {
        let mut enriched = sample();
        enriched.verdict.priorities = vec![];
        let html = render(&model(enriched));
        assert!(html.contains("Aucune."));
    }

    #[test]
    fn renders_commander_image_with_scryfall_link_when_printing_is_known() {
        let printing = ReferencePrinting {
            scryfall_id: "abc-123".to_string(),
            set_code: "M15".to_string(),
            number: "4".to_string(),
            is_two_faced: false,
        };
        let html = render(&ReportModel {
            commander_printing: Some(printing),
            ..model(sample())
        });
        assert!(
            html.contains("https://api.scryfall.com/cards/abc-123?format=image&amp;version=normal")
        );
        assert!(html.contains("https://scryfall.com/card/m15/4"));
    }

    #[test]
    fn falls_back_to_name_when_no_reference_printing() {
        let html = render(&model(sample()));
        assert!(!html.contains("scryfall.com"));
        assert!(html.contains("<h1>Atraxa, Praetors&#39; Voice</h1>"));
    }

    #[test]
    fn renders_suggestion_image_with_scryfall_link_when_printing_is_known() {
        let mut model = model(sample());
        model.suggestions[0].printing = Some(printing("rg-123", "M19", "191"));
        let html = render(&model);
        assert!(
            html.contains("https://api.scryfall.com/cards/rg-123?format=image&amp;version=normal")
        );
        assert!(html.contains("https://scryfall.com/card/m19/191"));
        assert!(html.contains("Rampant Growth"));
        assert!(html.contains("Comble le manque de ramp"));
    }

    #[test]
    fn falls_back_to_name_for_a_suggestion_without_reference_printing() {
        let html = render(&model(sample()));
        assert!(!html.contains("scryfall.com"));
        assert!(html.contains("Rampant Growth"));
    }

    #[test]
    fn renders_unchanged_without_a_card_to_remove() {
        let html = render(&model(sample()));
        assert!(!html.contains("class=\"card-to-remove\""));
    }

    #[test]
    fn renders_card_to_remove_smaller_with_its_own_image() {
        let mut enriched = sample();
        enriched.suggestions[0].card_to_remove = Some("Llanowar Elves".to_string());
        let mut model = model(enriched);
        model.suggestions[0].card_to_remove_printing = Some(printing("elves-123", "M19", "183"));
        let html = render(&model);
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
        let html = render(&model(enriched));
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn card_thumbnails_escape_the_card_name_exactly_once() {
        let mut enriched = sample();
        enriched.suggestions[0].card_name = r#"Foo "<Bar>""#.to_string();
        enriched.suggestions[0].card_to_remove = Some(r#"Baz "<Qux>""#.to_string());
        let mut model = model(enriched);
        model.suggestions[0].printing = Some(printing("foo-123", "TST", "1"));
        model.suggestions[0].card_to_remove_printing = Some(printing("baz-123", "TST", "2"));
        let html = render(&model);
        assert!(html.contains(r#"alt="Foo &quot;&lt;Bar&gt;&quot;""#));
        assert!(html.contains(r#"alt="Baz &quot;&lt;Qux&gt;&quot;""#));
        assert!(html.contains(r#"hidden>Foo &quot;&lt;Bar&gt;&quot;</span>"#));
        assert!(!html.contains("&amp;quot;"));
        assert!(!html.contains("<Bar>"));
        assert!(!html.contains("<Qux>"));
    }

    #[test]
    fn renders_no_warning_section_without_source_errors() {
        let html = render(&model(sample()));
        assert!(!html.contains("Avertissement"));
    }

    #[test]
    fn renders_a_warning_section_when_a_source_failed() {
        let mut enriched = sample();
        enriched.analysis.source_errors = vec![SourceError {
            source: "edhrec".to_string(),
            message: "HTTP 429".to_string(),
        }];
        let html = render(&model(enriched));
        assert!(html.contains("Avertissement"));
        assert!(html.contains("edhrec"));
        assert!(html.contains("HTTP 429"));
    }

    #[test]
    fn always_renders_credits_for_edhrec_and_recommander() {
        let html = render(&model(sample()));
        assert!(html.contains("https://edhrec.com"));
        assert!(html.contains("https://recommander.cards"));
    }

    #[test]
    fn appendix_lists_external_recommendations_not_turned_into_suggestions() {
        let mut enriched = sample();
        enriched.analysis.edhrec_recommendations = vec![EdhrecRecommendation {
            card: Card::named("Sol Ring", &[]),
            synergy: 0.42,
            inclusion_rate: 0.9,
            header: "High Synergy Cards".to_string(),
        }];
        let html = render(&model(enriched));
        assert!(html.contains("Recommandations externes non retenues"));
        assert!(html.contains("Sol Ring"));
        assert!(html.contains("0.42"));
    }

    #[test]
    fn appendix_excludes_external_recommendations_already_turned_into_suggestions() {
        let mut enriched = sample();
        enriched.analysis.edhrec_recommendations = vec![EdhrecRecommendation {
            card: Card::named("Rampant Growth", &[]),
            synergy: 0.42,
            inclusion_rate: 0.9,
            header: "High Synergy Cards".to_string(),
        }];
        // "Rampant Growth" est déjà une Suggestion retenue (voir `sample()`) :
        // l'annexe ne doit pas la lister une seconde fois.
        let html = render(&model(enriched));
        assert!(!html.contains("0.42"));
    }

    #[test]
    fn renders_origin_badges_for_a_suggestion() {
        let mut model = model(sample());
        model.suggestions[0].origins = vec![Origin::Kb, Origin::Edhrec];
        let html = render(&model);
        assert!(html.contains("badge-origin"));
        assert!(html.contains(">kb<"));
        assert!(html.contains(">edhrec<"));
    }

    #[test]
    fn renders_hover_preview_markup_once() {
        let html = render(&model(sample()));
        assert!(html.contains(r#"<div id="hover-preview" class="hover-preview" hidden>"#));
        assert!(html.contains(r#"<img class="hover-preview-face hover-preview-face-front""#));
        assert!(html.contains(r#"<img class="hover-preview-face hover-preview-face-back""#));
        assert!(html.contains("data-hover-img"));
        assert!(html.contains("addEventListener('mouseenter'"));
        assert!(html.contains("addEventListener('focus'"));
        assert!(html.contains("querySelector('img')"));
        // Une face en échec ne masque que celle-ci (pas l'infobulle entière) tant que
        // l'autre reste affichée — voir handleFaceError.
        assert!(html.contains("function handleFaceError(face, otherFace)"));
        assert!(html.contains("if (otherFace.hidden) {"));
    }

    #[test]
    fn two_faced_commander_thumbnail_carries_a_back_face_hover_attribute() {
        let printing = ReferencePrinting {
            scryfall_id: "delver-123".to_string(),
            set_code: "ISD".to_string(),
            number: "51".to_string(),
            is_two_faced: true,
        };
        let html = render(&ReportModel {
            commander_printing: Some(printing),
            ..model(sample())
        });
        assert!(html.contains(
            r#"data-hover-img-back="https://api.scryfall.com/cards/delver-123?format=image&amp;version=normal&amp;face=back""#
        ));
    }

    #[test]
    fn single_faced_commander_thumbnail_has_no_back_face_hover_attribute() {
        let printing = ReferencePrinting {
            scryfall_id: "abc-123".to_string(),
            set_code: "M15".to_string(),
            number: "4".to_string(),
            is_two_faced: false,
        };
        let html = render(&ReportModel {
            commander_printing: Some(printing),
            ..model(sample())
        });
        assert!(!html.contains(r#"data-hover-img-back="#));
    }

    #[test]
    fn synergy_card_with_known_printing_becomes_a_hover_link() {
        let mut model = model(sample());
        model.card_printings.insert(
            "Krenko, Mob Boss".to_string(),
            printing("krenko-123", "C17", "12"),
        );
        let html = render(&model);
        assert!(html.contains(
            r#"<a href="https://scryfall.com/card/c17/12" target="_blank" rel="noopener" class="hover-target" data-hover-img="https://api.scryfall.com/cards/krenko-123?format=image&amp;version=normal">Krenko, Mob Boss</a>"#
        ));
    }

    #[test]
    fn synergy_card_without_known_printing_stays_plain_text() {
        let html = render(&model(sample()));
        assert!(html.contains("Krenko, Mob Boss"));
        assert!(!html.contains(r#"data-hover-img="https://api.scryfall.com/cards/"#));
    }

    #[test]
    fn appendix_card_with_known_printing_becomes_a_hover_link() {
        let mut enriched = sample();
        enriched.analysis.edhrec_recommendations = vec![EdhrecRecommendation {
            card: Card::named("Sol Ring", &[]),
            synergy: 0.42,
            inclusion_rate: 0.9,
            header: "High Synergy Cards".to_string(),
        }];
        let mut model = model(enriched);
        model.card_printings.insert(
            "Sol Ring".to_string(),
            printing("solring-123", "CMR", "412"),
        );
        let html = render(&model);
        assert!(html.contains(
            r#"<a href="https://scryfall.com/card/cmr/412" target="_blank" rel="noopener" class="hover-target" data-hover-img="https://api.scryfall.com/cards/solring-123?format=image&amp;version=normal">Sol Ring</a>"#
        ));
        assert!(html.contains("synergie 0.42"));
    }

    #[test]
    fn commander_thumbnail_carries_the_hover_target_class() {
        let printing = ReferencePrinting {
            scryfall_id: "abc-123".to_string(),
            set_code: "M15".to_string(),
            number: "4".to_string(),
            is_two_faced: false,
        };
        let html = render(&ReportModel {
            commander_printing: Some(printing),
            ..model(sample())
        });
        assert!(html.contains("hover-target"));
        assert!(
            html.contains(
                r#"data-hover-img="https://api.scryfall.com/cards/abc-123?format=image&amp;version=normal""#
            )
        );
    }
}
