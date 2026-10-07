---
name: mtg-deck-analyze
description: Analyzes a Commander Deck (kb analyze → Verdict and Suggestions → kb report) and produces an HTML analysis Report. Use to analyze, evaluate or improve a Commander Decklist.
---

# mtg-deck-analyze

`kb` computes, you judge. To read a Card or search beyond the provided lists, use the `mtg-query` skill.

## Workflow

1. **Analyze**: `kb analyze <file|->` produces the JSON: Roles, `weaknesses`, Themes, Synergies, `candidates` and external Recommendations (`edhrec_recommendations`, `recommander_recommendations`, disabled by `--offline`).
   - Bracket: when the user gives a power level, pass `--bracket N` (1 to 5, the official Commander scale). Map a level described in words to its Bracket and confirm it with the user if unsure. Without a level, run without `--bracket`.
   - Missing or ambiguous Commander: fix the Decklist with the user, then rerun.
   - Report `unresolved` Cards and `source_errors` to the user; the analysis stays valid. Empty external Recommendations without `source_errors` are normal (Recommander returns nothing for a Decklist that is too short).

2. **Correct misclassified Cards**: `kb` detects Roles and Themes by oracle text patterns. When the oracle text of a Card in the Deck or in `candidates` contradicts its Role, Theme or Commander legality, set a Correction:
   - `kb override role|theme "<Card>" <v1,v2,…> --reason "<reason>"` (`--none` for an empty list);
   - `kb override legality "<Card>" legal|banned --reason "<reason>"`.

   The given list replaces the whole detection: enter the final list (a land that fights keeps `terrain`: `terrain,removal_cible`). Rerun `kb analyze` and continue from the new JSON.

3. **Pick Suggestions**: see [Choosing Suggestions](#choosing-suggestions). Every weakness in the JSON is addressed by a Suggestion or explained in the Verdict.

4. **Enrich the JSON**: add at the root of the `kb analyze` JSON, then save it to a temporary file:
   - `verdict`: `{"summary": "…", "strengths": […], "weaknesses": […], "priorities": […]}`, with a one- or two-sentence summary and priorities ordered most important first;
   - `suggestions`: `[{"card_name": "…", "justification": "…", "card_to_remove": "…"}]`, a one-sentence justification, `card_to_remove` optional (a Card from the Deck).

5. **Generate the Report**: `kb report <enriched json>` validates each Suggestion and writes `reports/<commander>-<date>.html`.
   - Refusal: fix each Suggestion it names and rerun. Game Changers are counted after the swaps, Suggestion by Suggestion in order: a `card_to_remove` that is a Game Changer frees a slot, so a Game Changer swapped for a Game Changer is always accepted. A Game Changer refused for exceeding the Bracket limit is either given a Game Changer from the Deck as `card_to_remove`, or replaced by a Card outside the Game Changer list that fills the same need.
   - Swap warning: `Grep` the Report for `class="swap-warning"`. Each hit is a swap that drops a Role with a threshold below its minimum. For each one, pick another `card_to_remove` and regenerate, or keep the swap and say why in its `justification` (the Role is covered elsewhere, the Deck's plan does not need it).

6. **Wrap-up**: give the Report path and list the Corrections set (Card, field, value, reason), or state that there were none.

## Choosing Suggestions

The number of Suggestions follows the Deck: as many as its weaknesses and Synergies justify.

Pool: `candidates`, external Recommendations and `kb search` (`--role`/`--theme` of the weakness, `--color-identity` of the Commander, `--legal-in commander`, `--exclude-deck <Decklist>`). Search with `kb search` as soon as the lists are thin or off-target. Keep a Card only if its oracle text fills a weakness or strengthens a specific Synergy from the JSON.

Rank the pool on evidence:
- **Origins**: each candidate and external Recommendation carries `origins` (`kb`, `edhrec`, `recommander`). Several Origins mark a consensus: favour those Cards. A single-Origin Card needs its oracle text to make the case on its own.
- **Signals** on every `card` (pool and Deck): `edhrec_rank` (lower is more played, absent for obscure Cards), `salt` (how much tables dislike it; weigh it against the user's table, never as a filter), `game_changer`. EDHREC Recommendations add `synergy`, `inclusion_rate`, and `theme` when they come from the page of a Major Theme rather than the Commander's.
- **Card to remove**: pick it from `cards` on the same signals: Roles and Themes it carries, mana value, `edhrec_rank`. Favour a Card with no Role under threshold and no Synergy.

Weaknesses that need a specific reading:
- `couleur sous-alimentée : <color> …`: the lands produce too few sources of that color for its share of mana symbols. Suggest lands that produce it, replacing lands that do not. Land candidates appear only when `base de mana insuffisante` is also present; otherwise search `kb search --role terrain` with `--text` on the color's mana symbol.
- `Game Changers au-delà de la limite du Bracket …`: the Deck holds more Game Changers than its Bracket allows. Suggest swaps whose `card_to_remove` is one of the listed Game Changers, and name the excess in the Verdict.

## When `kb` falls short

A need `kb` does not cover (missing or inconsistent data, missing computation or filter) gets reported rather than computed by hand. First look for an existing issue, open or closed, varying the keywords (`gh issue list --state all --search "<keywords>"`). If one exists, comment on it when you bring new context. Otherwise, open one with the Deck, the command run and the actual vs expected result. Either way, tell the user.
