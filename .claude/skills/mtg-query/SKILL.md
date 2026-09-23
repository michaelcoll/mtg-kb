---
name: mtg-query
description: Query the card database (kb search, kb set, kb rulings, kb card) and the rules database (kb rules) without ever writing raw SQL. Use whenever a question is about Cards, Sets, Rulings or the Comprehensive Rules — legality, color identity, type, oracle text, Rule number, Glossary term.
---

# mtg-query

`kb` gives typed access to the card database (`data/AllPrintings.sqlite`,
read-only) and the rules database (`data/rules.sqlite`, read-only). Never
query these SQLite files directly: always go through these subcommands,
which deduplicate Printings by oracle name and return stable JSON.

## Commands

- `kb card "<exact name>"` — one Card by exact oracle name, or by the name
  of its front face for a multi-face Card (`A // B`).
- `kb search [filters] [--limit N]` — combined search (all given filters
  combine with AND):
  - `--name <partial>`: substring of the name
  - `--type <partial>`: substring of the type line (e.g. `Creature`, `Legendary`)
  - `--subtype <partial>`: substring of the subtypes (e.g. `Elf`, `Equipment`)
  - `--text <partial>`: substring of the oracle text, repeatable (every
    occurrence must be present: `--text "target creature" --text destroy`
    only keeps Cards containing both)
  - `--color-identity <WUBRG>`: the Card's color identity is included in
    this set (e.g. `--color-identity GW` for a Selesnya Commander)
  - `--legal-in <format>`: legal in this format (`commander`, `standard`,
    `modern`, `pauper`, …)
  - `--mana-value <n>`: exact mana value
  - `--mana-value-min <n>` / `--mana-value-max <n>`: mana value range
    (inclusive bounds, combinable with `--mana-value` only if consistent)
  - `--role <role>`: detected Role (same detection as `kb analyze`: `ramp`,
    `pioche` (card draw), `removal_cible` (targeted removal), `wipe`,
    `protection`, `terrain` (land)), repeatable, combined with AND
  - `--theme <theme>`: detected Theme (same detection as `kb analyze`:
    `tokens`, `+1/+1`, `aristocrats`, `tribal:<subtype>`), repeatable,
    combined with AND
  - `--exclude-deck <file|->`: excludes the Cards in this Decklist
    (Commander and Deck), same parsing as `kb analyze`
- `kb set <code>` — Set information by code (`LEA`, `M19`, …)
- `kb rulings "<exact name>"` — dated Rulings of a Card, in chronological order
- `kb rules <number>` — a Rule (or a Section, e.g. `100`) and its
  sub-Rules (e.g. `kb rules 100.1`, `kb rules 702.19a`)
- `kb rules search "<text>"` — full-text search (FTS5, phrase search) in the
  Rules, with `--limit N`
- `kb rules define "<term>"` — definition of a Comprehensive Rules Glossary
  term

All accept `--format table` for human-readable terminal output; JSON is the
default and preferred for any programmatic use.

Roles, Themes and Commander legality reflect Corrections set with
`kb override` (see the `mtg-deck-analyze` skill), in every command.

## When to use them

- Check a Card's legality, color identity or oracle text before mentioning
  it in an analysis or a Suggestion.
- Explore a Set (size, release date, block).
- Look up a Card's Rulings history to clarify an interaction.
- Clarify a specific rules interaction (`kb rules <number>`) or find the
  exact definition of a term (`kb rules define`) before settling a judgement
  call in an analysis.
- Search beyond the `candidates` of `kb analyze`: filter by Role/Theme, mana
  value range or combined oracle text patterns, excluding Cards already in
  the Deck, to propose a Suggestion `kb analyze` did not surface (see the
  `mtg-deck-analyze` skill).

## What it does not do

- No prices or collection.
- No arbitrary SQL: if a filter is missing, add it to `kb search` rather
  than working around it with a direct query on the database.
