# kb calcule, Claude juge

L'analyse de deck est répartie entre deux parties. Le binaire `kb` fait tout ce qui est déterministe : lecture de la decklist, résolution des cartes, courbe de mana, base de mana, légalité, Rôles et Thèmes détectés par motifs de texte, et recherche de candidats. Il sort du JSON. Claude interprète ce JSON, choisit les Suggestions et rédige le verdict. `kb` produit ensuite le Rapport d'analyse HTML à partir du JSON enrichi.

On a écarté l'option « tout dans `kb` », car les suggestions seraient trop mécaniques. On a aussi écarté « tout côté Claude », car les chiffres ne seraient pas reproductibles et chaque analyse coûterait plus cher.
