---
name: mtg-db-update
description: Met à jour les bases locales (Base cartes MTGJSON et Base règles Wizards) via kb update. À déclencher UNIQUEMENT sur demande explicite de l'utilisateur (ex. "mets à jour la base cartes", "récupère les dernières règles") — jamais de façon proactive, car ces téléchargements remplacent des fichiers de données locaux.
---

# mtg-db-update

`kb update` télécharge et remplace les bases locales, sans jamais modifier le
schéma ni laisser de fichier corrompu en cas d'échec : téléchargement dans un
`.tmp`, validation, puis remplacement atomique. L'ancienne base est écrasée
sans copie `.bak`.

## Commandes

- `kb update cards` — télécharge `https://mtgjson.com/api/v5/AllPrintings.sqlite`
  dans `data/AllPrintings.tmp`, vérifie que la table `meta` est lisible et
  que sa `version` diffère de la base en place, puis remplace atomiquement
  `data/AllPrintings.sqlite`. Ne fait rien de plus si la version est déjà à
  jour (le `.tmp` est supprimé).
- `kb update rules [--url <url>]` — découvre (ou reçoit en argument) l'URL du
  document officiel Wizards, le télécharge, le découpe en Sections/Règles/
  Glossaire, construit `data/rules.tmp`, valide qu'elle s'ouvre correctement,
  puis remplace atomiquement `data/rules.sqlite`. Ne fait rien si la version
  (date embarquée dans le nom de fichier Wizards) est déjà celle en place.
- `kb update` (sans argument) — exécute les deux, cartes puis règles.

## Quand l'utiliser

Uniquement quand l'utilisateur le demande explicitement. Ne jamais lancer
une mise à jour de façon proactive : c'est un téléchargement de plusieurs
centaines de mégaoctets (Base cartes) qui remplace des données locales.

## Ce que ça ne fait pas

- Pas de sauvegarde `.bak` de l'ancienne base : le remplacement est
  définitif dès que la nouvelle base est validée.
- Pas de mise à jour partielle ou incrémentale : chaque `kb update`
  retélécharge la base entière.
