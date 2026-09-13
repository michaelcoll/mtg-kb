---
name: mtg-deck-analyze
description: Analyse un Deck Commander de bout en bout (kb analyze → interprétation et choix de Suggestions → kb report) et produit un Rapport d'analyse HTML. Utiliser quand l'utilisateur demande d'analyser, d'évaluer ou d'améliorer une Decklist Commander.
---

# mtg-deck-analyze

Orchestre le parcours complet d'analyse d'un Deck Commander, en suivant le
principe "kb calcule, Claude juge" : `kb` calcule tout ce qui est
déterministe, Claude interprète et juge.

## Parcours

1. **`kb analyze <fichier|->`** — résout la Decklist, calcule courbe de
   mana, base de mana, Rôles, Points faibles, Thèmes, Synergies, et une
   liste de `candidates` (Cartes légales Commander, dans l'Identité de
   couleur, absentes du Deck, classées par Thème et Rôle en Point faible).
   Sort du JSON. Voir le skill `mtg-query` pour vérifier des Cartes
   individuellement pendant l'analyse si besoin.

2. **Interprétation et choix** — lis le JSON produit. Rédige un `verdict`
   structuré (objet, pas une chaîne de texte) :
   `{"summary": "...", "strengths": ["..."], "weaknesses": ["..."], "priorities": ["..."]}`.
   `summary` en une ou deux phrases ; `strengths` (Points forts) et
   `weaknesses` (Faiblesses, une appréciation qualitative — peut
   s'appuyer sur les Points faibles mesurés par `kb` sans s'y limiter)
   en listes à puces ; `priorities` (Priorités) en liste ordonnée
   d'actions d'amélioration, la plus importante en premier. Choisis les
   `suggestions` parmi `candidates` (pas forcément tous : retiens celles
   qui comblent réellement un Point faible ou renforcent une Synergie),
   chacune avec une `justification` en une phrase et, si pertinent, une
   Carte à retirer : une Carte du Deck que la Suggestion propose de
   remplacer (`card_to_remove`, facultatif). Écris un JSON = celui de
   `kb analyze`, avec ces deux champs `verdict` et `suggestions` ajoutés
   au niveau racine (format
   `[{"card_name": "...", "justification": "...", "card_to_remove": "..."}]`
   pour `suggestions`, `card_to_remove` omis ou `null` si la Suggestion
   n'en propose pas). Sauvegarde ce JSON enrichi dans un fichier
   temporaire.

3. **`kb report <json enrichi>`** — valide chaque Suggestion (existe,
   légale en Commander, dans l'Identité de couleur du Commandant, absente
   du Deck, et sa Carte à retirer si renseignée fait partie du Deck) puis
   génère le Rapport d'analyse HTML autonome dans
   `reports/<commandant>-<date>.html` : Verdict (Résumé, Points forts,
   Faiblesses, Priorités), courbe, base de mana, Rôles, Points faibles,
   Synergies, Suggestions (avec justification et, le cas échéant, Carte
   à retirer affichée en plus petit avec son image), Cartes non
   résolues. Un `verdict` en chaîne de texte (ancien format) est rejeté
   avec une erreur claire — pas de rétrocompatibilité. Si une Suggestion
   viole une de ces contraintes, `kb report` échoue en nommant la Carte
   et la règle violée, sans écrire de rapport : corrige les
   `suggestions` du JSON enrichi et relance, ne contourne pas l'échec.

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
- Si `kb` ne couvre pas un besoin rencontré pendant l'analyse (donnée
  manquante ou incohérente, calcul absent, candidats trop peu nombreux
  ou mal ciblés, etc.), ne compense pas en le faisant manuellement à la
  place de `kb` — ouvre une issue sur le repo (voir
  `docs/agents/issue-tracker.md`) décrivant le manque et son contexte
  (Deck concerné, commande lancée, résultat obtenu vs attendu), puis
  signale-le à l'utilisateur.
