---
name: mtg-deck-analyze
description: Analyzes a Commander Deck (kb analyze → Verdict and Suggestions → kb report) and produces an HTML analysis Report. Use to analyze, evaluate or improve a Commander Decklist.
---

# mtg-deck-analyze

`kb` computes, you judge. To read a Card or search beyond the provided lists, use the `mtg-query` skill.

## Workflow

1. **Analyze**: `kb analyze <file|->` produces the JSON: Roles, weaknesses, Themes, Synergies, `candidates` and external Recommendations (`edhrec_recommendations`, `recommander_recommendations`, disabled by `--offline`).
   - Missing or ambiguous Commander: fix the Decklist with the user, then rerun.
   - Report `unresolved` Cards and `source_errors` to the user; the analysis stays valid. Empty external Recommendations without `source_errors` are normal (Recommander returns nothing for a Decklist that is too short).

2. **Correct misclassified Cards**: `kb` detects Roles and Themes by oracle text patterns. When the oracle text of a Card in the Deck or in `candidates` contradicts its Role, Theme or Commander legality, set a Correction:
   - `kb override role|theme "<Card>" <v1,v2,…> --reason "<reason>"` (`--none` for an empty list);
   - `kb override legality "<Card>" legal|banned --reason "<reason>"`.

   The given list replaces the whole detection: enter the final list (a land that fights keeps `terrain`: `terrain,removal_cible`). Rerun `kb analyze` and continue from the new JSON.

3. **Pick Suggestions**: draw equally from `candidates`, external Recommendations and `kb search` (`--role`/`--theme` of the weakness, `--color-identity` of the Commander, `--legal-in commander`, `--exclude-deck <Decklist>`). Search with `kb search` as soon as the lists are thin or off-target. Keep a Card only if its oracle text fills a weakness or strengthens a specific Synergy from the JSON.

4. **Enrich the JSON**: add at the root of the `kb analyze` JSON, then save it to a temporary file:
   - `verdict`: `{"summary": "…", "strengths": […], "weaknesses": […], "priorities": […]}`, with a one- or two-sentence summary and priorities ordered most important first;
   - `suggestions`: `[{"card_name": "…", "justification": "…", "card_to_remove": "…"}]`, a one-sentence justification, `card_to_remove` optional (a Card from the Deck).

5. **Generate the Report**: `kb report <enriched json>` validates each Suggestion and writes `reports/<commander>-<date>.html`. If it fails, fix the `suggestions` it names and rerun.

6. **Wrap-up**: give the Report path and list the Corrections set (Card, field, value, reason), or state that there were none.

## When `kb` falls short

A need `kb` does not cover (missing or inconsistent data, missing computation or filter) gets reported rather than computed by hand. First look for an existing issue, open or closed, varying the keywords (`gh issue list --state all --search "<keywords>"`). If one exists, comment on it when you bring new context. Otherwise, open one with the Deck, the command run and the actual vs expected result. Either way, tell the user.
