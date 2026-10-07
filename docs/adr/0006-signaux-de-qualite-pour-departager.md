# Signaux de qualité : kb départage, Claude juge

`kb analyze` expose par Carte des signaux de qualité (mana value, rang de popularité EDHREC, salt, Game Changer, Origines) et ne s'en sert que pour départager les Candidats à score égal et plafonner les listes. Il ne produit pas de classement final pondéré, et ne choisit pas la Carte à retirer. `kb report` simule seulement chaque échange et avertit, sans bloquer, quand un Rôle repasse sous son seuil.

Cela prolonge l'ADR 0001. Sans critère de départage, les 10 Candidats d'un panier étaient les premiers de l'alphabet parmi des centaines d'égalités. Un tri déterministe pour couper une liste reste du calcul, alors qu'un classement pondéré figerait un jugement dans `kb` et rendrait les Suggestions mécaniques.

Le Bracket ne fait respecter que la limite de Game Changers, seule règle que la Base cartes porte de façon fiable (`isGameChanger`). La destruction massive de terrains, les tours supplémentaires et les combos ne sont pas détectés : des regex seraient trop imprécises (cf. ADR 0005), et Claude les juge à partir du texte oracle.

On a écarté :

- un classement final calculé par `kb`, que Claude se contenterait de valider ;
- une liste « Cartes faibles » proposée au retrait par `kb` ;
- un blocage de `kb report` sur un échange qui affaiblit un Rôle, car le choix peut être voulu ;
- un filtre par salt, qui n'est pas une règle officielle.
