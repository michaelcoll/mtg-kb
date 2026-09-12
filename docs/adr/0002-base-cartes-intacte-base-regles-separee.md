# Base cartes MTGJSON intacte, Base règles séparée

`kb` ne modifie jamais `AllPrintings.sqlite` : il la lit telle que MTGJSON la publie, et `kb update cards` la remplace entière. Les Règles complètes sont stockées dans une base à part, `rules.sqlite`, avec leurs propres index FTS5 et glossaire. Ainsi, une mise à jour MTGJSON ne peut ni effacer les règles ni casser un schéma qu'on aurait modifié. Chaque base se met à jour indépendamment de l'autre.
