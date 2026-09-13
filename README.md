# mtg-kb

Base de connaissances Magic: The Gathering locale (cartes, sets, règles) et
outillage d'analyse de decks Commander, exposée via le CLI `kb`.

## Prérequis

- [mise](https://mise.jdx.dev/) : gère la version de Rust et les tâches du
  projet (voir `mise.toml`).

Installer la toolchain déclarée dans `mise.toml` :

```sh
mise install
```

## Installer le CLI

```sh
mise run install   # alias: mise run i
```

Compile en mode release et installe le binaire `kb` dans `~/.cargo/bin`
(via `cargo install --path .`). Assurez-vous que `~/.cargo/bin` est dans
votre `PATH`.

## Données locales

Le CLI lit deux bases SQLite en lecture seule, par défaut dans `data/` (le
dossier peut être changé avec la variable d'environnement `KB_DATA_DIR`) :

- `AllPrintings.sqlite` — Base cartes (source : MTGJSON)
- `rules.sqlite` — Base règles (Comprehensive Rules)

Pour les télécharger ou les mettre à jour :

```sh
kb update          # cartes + règles
kb update cards
kb update rules [--url <url>]
```

## Utiliser le CLI

Interroger la Base cartes et la Base règles :

```sh
kb card "Sol Ring"
kb search --type Creature --color-identity WU --legal-in commander
kb set NEO
kb rulings "Sol Ring"
kb rules search "commander tax"
kb rules define "Trample"
kb rules 702.19
```

Analyser une Decklist Commander (fichier ou stdin) :

```sh
kb analyze decks/ma-decklist.txt
```

Produit un JSON déterministe (courbe de mana, base de mana, Rôles, Points
faibles, Thèmes, Synergies, candidats aux Suggestions). Génère ensuite le
Rapport d'analyse HTML à partir d'un JSON enrichi (verdict + Suggestions
retenues, voir le skill `mtg-deck-analyze`) :

```sh
kb report deck-enrichi.json
```

## Développement

Tâches disponibles via `mise run <tâche>` (voir `mise.toml` pour la liste
complète) :

```sh
mise run build     # cargo build --release   (alias: b)
mise run test      # cargo nextest run        (alias: t)
mise run lint      # cargo clippy --all-targets -- -D warnings
mise run format    # cargo fmt                (alias: f)
mise run checks    # test + lint + fmt-check
```

## Pour aller plus loin

- `CONTEXT.md` — vocabulaire du domaine (Carte, Deck, Rôle, Thème, Point
  faible, Suggestion, …)
- `docs/adr/` — décisions d'architecture
- `.claude/skills/` — skills agent (`mtg-query`, `mtg-deck-analyze`,
  `mtg-db-update`)
