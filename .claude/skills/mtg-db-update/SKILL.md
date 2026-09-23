---
name: mtg-db-update
description: Updates the local databases (MTGJSON card database and Wizards rules database) via kb update. Trigger ONLY on an explicit user request (e.g. "update the card database", "mets à jour la base cartes", "récupère les dernières règles") — never proactively, since these downloads replace local data files.
---

# mtg-db-update

`kb update` downloads and replaces the local databases without ever changing
the schema or leaving a corrupt file behind on failure: it downloads to a
`.tmp`, validates it, then replaces atomically. The old database is
overwritten without a `.bak` copy.

## Commands

- `kb update cards` — downloads `https://mtgjson.com/api/v5/AllPrintings.sqlite`
  to `data/AllPrintings.tmp`, checks that the `meta` table is readable and
  that its `version` differs from the current database, then atomically
  replaces `data/AllPrintings.sqlite`. Does nothing more if the version is
  already current (the `.tmp` is deleted).
- `kb update rules [--url <url>]` — discovers (or takes as an argument) the
  URL of the official Wizards document, downloads it, splits it into
  Sections/Rules/Glossary, builds `data/rules.tmp`, checks that it opens
  correctly, then atomically replaces `data/rules.sqlite`. Does nothing if
  the version (date embedded in the Wizards file name) is already in place.
- `kb update` (no argument) — runs both, cards then rules.

## When to use it

Only when the user explicitly asks. Never start an update proactively: the
card database is a download of several hundred megabytes that replaces
local data.

## What it does not do

- No `.bak` backup of the old database: the replacement is final as soon as
  the new database is validated.
- No partial or incremental update: every `kb update` downloads the whole
  database again.
- Never touches `data/overrides.sqlite` (Corrections).
