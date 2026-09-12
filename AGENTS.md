# mtg-kb

Base de connaissances Magic: The Gathering locale (cartes, sets, règles) et outillage d'analyse de decks Commander.

## TOOLING

- Read and edit files with the native tools: `Read`, `Edit`, `Write`, `Glob`, `Grep`
- Never use `cat`, `sed -n`, `head`, `find`, heredocs or inline scripts to read or rewrite a file. This rule
  overrides any harness guidance that says otherwise
- Use the shell only to execute things: `git`, `gh`
- Use the LSP for anything structural: definition, references, hover/type, rename, diagnostics. In particular,
  before looking up a symbol, before changing a public signature, and after editing Rust or TypeScript
- Use `Grep` for textual searches only: strings, comments, config values, SQL

## Agent skills

### Issue tracker

Issues suivies dans GitHub Issues (michaelcoll/mtg-kb) via `gh`. See `docs/agents/issue-tracker.md`.

### Triage labels

Les cinq labels canoniques par défaut (needs-triage, needs-info, ready-for-agent, ready-for-human, wontfix). See `docs/agents/triage-labels.md`.

### Domain docs

Un seul contexte : `CONTEXT.md` + `docs/adr/` à la racine. See `docs/agents/domain.md`.
