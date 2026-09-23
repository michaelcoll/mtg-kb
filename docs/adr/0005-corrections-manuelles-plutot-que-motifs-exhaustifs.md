# Corrections manuelles plutôt que motifs exhaustifs

`kb` détecte les Rôles et les Thèmes par des motifs de texte oracle. Ces motifs se trompent : des sweepers asymétriques ne sont pas reconnus, alors que la destruction de ses propres tokens est prise pour un wipe (#54, #69, #75). Couvrir chaque tournure alourdit les regex sans jamais atteindre l'exhaustivité. Les Rôles, les Thèmes et la légalité Commander d'une Carte peuvent donc être fixés par une Correction, posée avec `kb override`. On arrête d'enrichir les motifs à chaque cas isolé.

Les Corrections sont stockées dans `data/overrides.sqlite`, une base à part que `kb update` ne touche jamais. Cela prolonge l'ADR 0002 : la Base cartes reste intacte. Une Correction porte sur une Carte entière, désignée par son nom oracle. Elle **remplace** la valeur détectée au lieu de l'ajuster, et elle exige un motif. `CardsDb` l'applique au moment où il construit la Carte (ADR 0004), si bien que `kb analyze`, les Candidats, `kb search` et `kb card` voient tous la même valeur. Le JSON indique quels champs sont corrigés.

C'est Claude qui pose les Corrections, pendant l'analyse d'un Deck, puis il les liste dans son compte rendu. On nuance ainsi l'ADR 0001 : le jugement de Claude peut désormais corriger ce que `kb` déduit, alors que les calculs de `kb` restent déterministes pour un même état de la base.

On a écarté deux options :

- continuer à enrichir les motifs, parce que le coût de maintenance ne cesse de croître ;
- un fichier versionné, parce qu'on a préféré une base à part, sur le modèle de `rules.sqlite`.

Conséquence acceptée : une Correction ne corrige qu'une seule Carte. Les Cartes du pool des Candidats qui échappent aux motifs restent mal classées tant que personne ne les a corrigées.
