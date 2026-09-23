---
name: mtg-query
description: Interroger la Base cartes (kb search, kb set, kb rulings, kb card) et la Base règles (kb rules) sans jamais écrire de SQL brut. Utiliser dès qu'une question porte sur des Cartes, Sets, Rulings ou les Comprehensive Rules — légalité, identité de couleur, type, texte oracle, numéro de Règle, terme de Glossaire.
---

# mtg-query

`kb` expose un accès typé à la Base cartes (`data/AllPrintings.sqlite`,
lecture seule) et à la Base règles (`data/rules.sqlite`, lecture seule).
N'interroge jamais ces fichiers SQLite directement : passe toujours par ces
sous-commandes, qui dédoublonnent les Impressions par nom oracle et
renvoient du JSON stable.

## Commandes

- `kb card "<nom exact>"` — une Carte par nom oracle exact (cartes double
  face : nom complet `A // B`).
- `kb search [filtres] [--limit N]` — recherche combinée (tous les filtres
  fournis se combinent en ET) :
  - `--name <partiel>` : sous-chaîne du nom
  - `--type <partiel>` : sous-chaîne de la ligne de type (ex. `Creature`, `Legendary`)
  - `--subtype <partiel>` : sous-chaîne des sous-types (ex. `Elf`, `Equipment`)
  - `--text <partiel>` : sous-chaîne du texte oracle, répétable (chaque
    occurrence doit être présente : `--text "target creature" --text destroy`
    ne retient que les Cartes qui contiennent les deux)
  - `--color-identity <WUBRG>` : Identité de couleur de la Carte incluse dans
    cet ensemble (ex. `--color-identity GW` pour un Commandant Selesnya)
  - `--legal-in <format>` : légal dans ce format (`commander`, `standard`,
    `modern`, `pauper`, …)
  - `--mana-value <n>` : mana value exacte
  - `--mana-value-min <n>` / `--mana-value-max <n>` : plage de mana value
    (bornes incluses, combinable avec `--mana-value` seulement si
    cohérente)
  - `--role <rôle>` : Rôle détecté (même détection que `kb analyze` :
    `ramp`, `pioche`, `removal_cible`, `wipe`, `protection`, `terrain`),
    répétable, combiné en ET. Une Correction (`kb override`) remplace les
    Rôles, Thèmes et la légalité Commander détectés, ici comme partout
    ailleurs
  - `--theme <thème>` : Thème détecté (même détection que `kb analyze` :
    `tokens`, `+1/+1`, `aristocrats`, `tribal:<sous-type>`), répétable,
    combiné en ET
  - `--exclude-deck <fichier|->` : exclut les Cartes présentes dans cette
    Decklist (Commandant et Deck), même parsing que `kb analyze`
- `kb set <code>` — informations d'un Set par code (`LEA`, `M19`, …)
- `kb rulings "<nom exact>"` — Rulings datés d'une Carte, triés chronologiquement
- `kb rules <numéro>` — une Règle (ou une Section, ex. `100`) et ses
  sous-Règles (ex. `kb rules 100.1`, `kb rules 702.19a`)
- `kb rules search "<texte>"` — recherche plein texte (FTS5, recherche de
  phrase) dans les Règles, avec `--limit N`
- `kb rules define "<terme>"` — définition d'un terme du Glossaire des
  Comprehensive Rules

Toutes acceptent `--format table` pour une sortie lisible en terminal ;
JSON par défaut, à préférer pour tout traitement programmatique.

## Quand les utiliser

- Vérifier la légalité, l'Identité de couleur ou le texte oracle d'une Carte
  avant de la mentionner dans une analyse ou une Suggestion.
- Explorer un Set (taille, date de sortie, bloc).
- Retrouver l'historique des Rulings d'une Carte pour clarifier une
  interaction.
- Clarifier une interaction de règles précise (`kb rules <numéro>`) ou
  retrouver la définition exacte d'un terme (`kb rules define`) avant de
  trancher un point de jugement dans une analyse.
- Investiguer librement au-delà des `candidates` de `kb analyze` : filtrer
  par Rôle/Thème, plage de mana value ou motifs de texte oracle combinés,
  en excluant les Cartes déjà dans le Deck, pour proposer une Suggestion
  que `kb analyze` n'a pas remontée (voir le skill `mtg-deck-analyze`).

## Ce que ça ne fait pas

- Pas de prix ni de collection.
- Pas de SQL arbitraire : si un filtre manque, l'ajouter à `kb search`
  plutôt que de contourner via une requête directe sur la base.
