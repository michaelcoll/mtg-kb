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
   Interroge aussi par défaut deux Sources externes (EDHREC et
   Recommander, voir ADR 0003) et expose leurs Recommandations externes
   dans `edhrec_recommendations` et `recommander_recommendations` : mêmes
   filtres que `candidates` (légale en Commander, dans l'Identité de
   couleur, absente du Deck), au plus 30 par Source. `--offline` désactive
   ces appels (listes vides). Sort du JSON. Voir le skill `mtg-query` pour
   vérifier des Cartes individuellement pendant l'analyse si besoin.

   Si `source_errors` n'est pas vide, une Source externe a échoué (réseau,
   HTTP 429, structure inattendue) : l'analyse reste complète, mais
   `edhrec_recommendations` et/ou `recommander_recommendations` peuvent
   être incomplètes ou vides. Signale-le à l'utilisateur ; ne bloque pas
   l'analyse pour autant.

2. **Interprétation et choix** — lis le JSON produit. Rédige un `verdict`
   structuré (objet, pas une chaîne de texte) :
   `{"summary": "...", "strengths": ["..."], "weaknesses": ["..."], "priorities": ["..."]}`.
   `summary` en une ou deux phrases ; `strengths` (Points forts) et
   `weaknesses` (Faiblesses, une appréciation qualitative — peut
   s'appuyer sur les Points faibles mesurés par `kb` sans s'y limiter)
   en listes à puces ; `priorities` (Priorités) en liste ordonnée
   d'actions d'amélioration, la plus importante en premier.

   Construis les `suggestions` à partir de `candidates`, des
   Recommandations externes (`edhrec_recommendations`,
   `recommander_recommendations`) **et**, quand c'est utile, d'une
   investigation libre — les trois sources sont traitées à égalité, aucun
   quota ne s'applique à l'une ou l'autre. Retiens uniquement les Cartes
   qui comblent réellement un Point faible ou renforcent une Synergie ;
   ignore les `candidates` et Recommandations externes qui n'en comblent
   aucun — ce ne sont pas des Suggestions à recopier telles quelles, elles
   restent soumises au même jugement que le reste.

   **Investigue au-delà de `candidates`** dès que ces derniers semblent
   insuffisants ou mal ciblés pour un Point faible ou un Thème majeur du
   JSON (peu de candidats pertinents, aucun dans la bonne fourchette de
   mana value, tous redondants avec l'existant, etc.). Utilise
   `kb search` (skill `mtg-query`) avec les filtres pertinents : `--role`
   ou `--theme` pour cibler le Point faible ou le Thème majeur en
   question, `--color-identity` (celle du Commandant), `--legal-in
   commander`, `--mana-value-min`/`--mana-value-max` pour la courbe,
   `--exclude-deck <même Decklist>` pour ne pas proposer une Carte déjà
   présente. Une Suggestion issue de cette investigation est traitée
   exactement comme une Suggestion issue de `candidates` pour la suite.

   **Vérifie le texte oracle avant de retenir toute Suggestion**, qu'elle
   vienne de `candidates` ou de l'investigation : lis-le (`kb card` ou le
   champ déjà présent dans `candidates`) et confirme que la Carte fait
   réellement ce que son Rôle/Thème détecté prétend. Un tag
   `removal_cible` ou un Thème détecté par motif ne suffit pas à lui
   seul — la détection de `kb` est faite de motifs textuels, pas d'une
   compréhension de l'effet ; une Carte mal taguée ne doit pas devenir
   une Suggestion.

   Pour chaque Suggestion retenue, rédige une `justification` en une
   phrase et, si pertinent, une Carte à retirer : une Carte du Deck que
   la Suggestion propose de remplacer (`card_to_remove`, facultatif).
   Écris un JSON = celui de `kb analyze`, avec ces deux champs `verdict`
   et `suggestions` ajoutés au niveau racine (format
   `[{"card_name": "...", "justification": "...", "card_to_remove": "..."}]`
   pour `suggestions`, `card_to_remove` omis ou `null` si la Suggestion
   n'en propose pas). Sauvegarde ce JSON enrichi dans un fichier
   temporaire.

3. **`kb report <json enrichi>`** — valide chaque Suggestion (existe,
   légale en Commander, dans l'Identité de couleur du Commandant, absente
   du Deck, et sa Carte à retirer si renseignée fait partie du Deck),
   qu'elle vienne de `candidates`, d'une Recommandation externe ou de
   l'investigation — la validation ne distingue pas la source — puis
   calcule l'Origine de chaque Suggestion (`kb`, `edhrec`, `recommander`,
   `investigation`, éventuellement plusieurs — Claude ne renseigne rien
   ici) et génère le Rapport d'analyse HTML autonome dans
   `reports/<commandant>-<date>.html` : Verdict (Résumé, Points forts,
   Faiblesses, Priorités), avertissement si `source_errors` n'est pas
   vide, courbe, base de mana, Rôles, Points faibles, Synergies,
   Suggestions (avec justification, badges d'Origine et, le cas échéant,
   Carte à retirer affichée en plus petit avec son image), une annexe des
   Recommandations externes filtrées non retenues en Suggestion, crédit
   et lien vers EDHREC et Recommander, Cartes non résolues. Un `verdict`
   en chaîne de texte (ancien format) est rejeté avec une erreur claire —
   pas de rétrocompatibilité. Si une Suggestion viole une de ces
   contraintes, `kb report` échoue en nommant la Carte et la règle
   violée, sans écrire de rapport : corrige les `suggestions` du JSON
   enrichi et relance, ne contourne pas l'échec.

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
- Des `candidates` ou Recommandations externes trop peu nombreux ou mal
  ciblés ne sont pas un manque de `kb` : c'est le signal d'investiguer via
  `kb search` (voir l'étape 2).
- `edhrec_recommendations`/`recommander_recommendations` vides sans
  `source_errors` associée n'est pas forcément une panne : Recommander
  renvoie une liste vide si la Decklist est trop courte, EDHREC est par
  Commandant seul (indépendant du reste du Deck).
- Si `kb` ne couvre pas un besoin rencontré pendant l'analyse (donnée
  manquante ou incohérente, calcul absent, filtre de `kb search` qui
  manquerait pour une investigation, etc.), ne compense pas en le
  faisant manuellement à la place de `kb`. Avant d'ouvrir une issue,
  cherche un ticket existant, ouvert ou fermé, décrivant le même manque
  (`gh issue list --state all --search "<mots-clés>"`, voir
  `docs/agents/issue-tracker.md`) : varie les mots-clés (nom de la
  commande `kb`, nom de la Carte ou du champ concerné) si la première
  recherche ne remonte rien. Si un ticket correspond, ne le recrée pas —
  ajoute un commentaire s'il apporte un contexte nouveau (nouveau Deck
  concerné, nouvelle commande reproduisant le même symptôme), sinon
  contente-toi de le signaler à l'utilisateur. Seulement si aucun ticket
  existant ne correspond, ouvre-en un décrivant le manque et son contexte
  (Deck concerné, commande lancée, résultat obtenu vs attendu), puis
  signale-le à l'utilisateur.
