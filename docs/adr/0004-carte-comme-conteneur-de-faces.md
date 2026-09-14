# Carte modélisée comme conteneur de Faces

`kb` ne géraient jusqu'ici qu'une seule Face par Carte (une ligne SQL arbitraire sur les Cartes multi-faces), ce qui perdait des Rôles/Thèmes et faussait `mana_base` pour toute Carte transform/modal_dfc/split/adventure/aftermath/flip (#45). On modélise désormais `Card` comme un conteneur `faces: Vec<Face>` (1 élément pour une Carte normale, 2 pour une Carte multi-face) plutôt qu'un type séparé `MultiFaceCard`, pour garder un seul chemin de code dans `detect_roles`/`detect_themes`/`mana_base`/l'affichage. Les Rôles et Thèmes d'une Carte multi-face sont l'union de ceux de ses Faces, faute de pouvoir savoir statiquement laquelle sera jouée. Le périmètre couvre tous les layouts MTGJSON où `name` combine deux Faces (transform, modal_dfc, split, adventure, aftermath, flip) ; `meld` est explicitement exclu, car il s'agit de deux Cartes physiques distinctes et non de deux Faces d'une même Carte.

## Considered Options

- Type séparé `MultiFaceCard { front, back }` à côté de `Card` : rejeté, aurait introduit deux chemins de code à maintenir dans toute la chaîne d'analyse.
- Ne compter que le Rôle/Thème de la Face avant : rejeté, contredit l'attente de l'issue (les deux Faces sont jouables) et n'a pas de fondement pour choisir arbitrairement une Face.

## Consequences

- `color_identity` n'est pas affecté par ce changement : la colonne MTGJSON `colorIdentity` est déjà l'union des deux Faces, identique sur les deux lignes SQL.
- Les compteurs `role_counts`/`theme_counts`/nombre de terrains dans les Rapports d'analyse existants peuvent changer (à la hausse, correction d'un sous-comptage) après ce fix.
