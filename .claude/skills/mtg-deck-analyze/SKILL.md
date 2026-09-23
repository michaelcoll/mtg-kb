---
name: mtg-deck-analyze
description: Analyse un Deck Commander (kb analyze → Verdict et Suggestions → kb report) et produit un Rapport d'analyse HTML. Utiliser pour analyser, évaluer ou améliorer une Decklist Commander.
---

# mtg-deck-analyze

`kb` calcule, tu juges. Pour lire une Carte ou chercher au-delà des listes fournies, utilise le skill `mtg-query`.

## Parcours

1. **Analyser** : `kb analyze <fichier|->` produit le JSON : Rôles, Points faibles, Thèmes, Synergies, `candidates` et Recommandations externes (`edhrec_recommendations`, `recommander_recommendations`, désactivées par `--offline`).
   - Commandant absent ou ambigu : corrige la Decklist avec l'utilisateur, puis relance.
   - Signale à l'utilisateur les Cartes `unresolved` et les `source_errors` ; l'analyse reste valable. Des Recommandations externes vides sans `source_errors` sont normales (Recommander ne répond rien pour une Decklist trop courte).

2. **Corriger les Cartes mal classées** : `kb` détecte Rôles et Thèmes par motifs de texte oracle. Quand le texte oracle d'une Carte du Deck ou des `candidates` contredit son Rôle, son Thème ou sa légalité Commander, pose une Correction :
   - `kb override role|theme "<Carte>" <v1,v2,…> --reason "<motif>"` (`--none` pour une liste vide) ;
   - `kb override legality "<Carte>" legal|banned --reason "<motif>"`.

   La liste donnée remplace toute la détection : saisis la liste finale (un terrain qui fight garde `terrain` : `terrain,removal_cible`). Relance `kb analyze` et repars du nouveau JSON.

3. **Choisir les Suggestions** : puise à égalité dans `candidates`, les Recommandations externes et `kb search` (`--role`/`--theme` du Point faible, `--color-identity` du Commandant, `--legal-in commander`, `--exclude-deck <Decklist>`). Cherche avec `kb search` dès que les listes sont maigres ou mal ciblées. Retiens une Carte seulement si son texte oracle comble un Point faible ou renforce une Synergie précise du JSON.

4. **Enrichir le JSON** : ajoute à la racine du JSON de `kb analyze`, puis sauvegarde-le dans un fichier temporaire :
   - `verdict` : `{"summary": "…", "strengths": […], "weaknesses": […], "priorities": […]}`, avec un Résumé d'une ou deux phrases et les Priorités ordonnées, la plus importante d'abord ;
   - `suggestions` : `[{"card_name": "…", "justification": "…", "card_to_remove": "…"}]`, une justification d'une phrase, `card_to_remove` facultatif (une Carte du Deck).

5. **Générer le Rapport** : `kb report <json enrichi>` valide chaque Suggestion et écrit `reports/<commandant>-<date>.html`. S'il échoue, corrige les `suggestions` qu'il nomme et relance.

6. **Compte rendu** : donne le chemin du Rapport et liste les Corrections posées (Carte, champ, valeur, motif), ou indique qu'il n'y en a aucune.

## Quand `kb` ne suffit pas

Un besoin que `kb` ne couvre pas (donnée manquante ou incohérente, calcul ou filtre absent) se signale plutôt que de se calculer à la main. Cherche d'abord une issue existante, ouverte ou fermée, en variant les mots-clés (`gh issue list --state all --search "<mots-clés>"`). Si elle existe, commente-la quand tu apportes un contexte nouveau. Sinon, ouvre-en une avec le Deck, la commande lancée et le résultat obtenu vs attendu. Dans les deux cas, préviens l'utilisateur.
