---
name: mtg-deck-analyze
description: Analyse un Deck Commander de bout en bout (kb analyze → interprétation et choix de Suggestions → kb report) et produit un Rapport d'analyse HTML. Utiliser quand l'utilisateur demande d'analyser, d'évaluer ou d'améliorer une Decklist Commander.
---

# mtg-deck-analyze

Orchestre le parcours complet d'analyse d'un Deck Commander, en suivant
l'ADR "kb calcule, Claude juge" ([[kb-deterministe-claude-jugement]]) :
`kb` calcule tout ce qui est déterministe, Claude interprète et juge.

## Parcours

1. **`kb analyze <fichier|->`** — résout la Decklist, calcule courbe de
   mana, base de mana, Rôles, Points faibles, Thèmes, Synergies, et une
   liste de `candidates` (Cartes légales Commander, dans l'Identité de
   couleur, absentes du Deck, classées par Thème et Rôle en Point faible).
   Sort du JSON. Voir le skill `mtg-query` pour vérifier des Cartes
   individuellement pendant l'analyse si besoin.

2. **Interprétation et choix** — lis le JSON produit. Rédige un `verdict`
   (une évaluation en prose du Deck : forces, faiblesses, cohérence de
   stratégie). Choisis les `suggestions` parmi `candidates` (pas
   forcément tous : retiens celles qui comblent réellement un Point
   faible ou renforcent une Synergie), chacune avec une `justification`
   en une phrase. Écris un JSON = celui de `kb analyze`, avec ces deux
   champs `verdict` et `suggestions` ajoutés au niveau racine (format
   `[{"card_name": "...", "justification": "..."}]` pour `suggestions`).
   Sauvegarde ce JSON enrichi dans un fichier temporaire.

3. **`kb report <json enrichi>`** — génère le Rapport d'analyse HTML
   autonome dans `reports/<commandant>-<date>.html` : résumé/verdict,
   courbe, base de mana, Rôles, Points faibles, Synergies, Suggestions
   (avec justification), Cartes non résolues.

## Points d'attention

- Si `kb analyze` échoue (Commandant absent ou ambigu), corrige la
  Decklist avec l'utilisateur avant de continuer — ne fabrique pas de
  JSON à la main pour contourner l'erreur.
- Les Cartes non résolues n'empêchent pas l'analyse ; signale-les à
  l'utilisateur, elles manquent probablement au verdict.
- Ne choisis pas plus de Suggestions que nécessaire : chaque Suggestion
  doit se justifier par un Point faible ou une Synergie précis du JSON,
  pas par une préférence générique.
- Le design du Rapport HTML est un MVP ; d'autres itérations de design
  sont prévues séparément — ne pas sur-investir dans le style ici.
