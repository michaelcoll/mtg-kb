# MTG Knowledge Base

Base de connaissances Magic: The Gathering locale, utilisée pour interroger cartes, sets et règles, et pour analyser et améliorer des decks Commander.

## Données de référence

**Carte**:
Une carte Magic identifiée par son nom oracle unique, indépendamment de ses impressions ; porte le texte, le coût, les types, l'identité de couleur et la légalité.
_Avoid_: Card print, fiche

**Impression**:
Une édition précise d'une Carte dans un Set (identifiée par un uuid MTGJSON) ; l'analyse de deck n'en tient pas compte.
_Avoid_: Printing, version, variante

**Impression de référence**:
L'Impression retenue pour illustrer une Carte dans le Rapport d'analyse : la plus récente en papier, hors promo, hors format surdimensionné et hors cartes fantaisie ; elle n'influence pas l'analyse.
_Avoid_: Image, illustration

**Set**:
Une extension ou un produit publié regroupant des Impressions.
_Avoid_: Édition, extension

**Ruling**:
Une clarification officielle datée portant sur une Carte précise.
_Avoid_: Règle, FAQ

**Règles complètes**:
Le document officiel des Comprehensive Rules, découpé en Règles numérotées (ex. 702.19) et en Entrées de glossaire.
_Avoid_: CR, rulebook

**Base cartes**:
La base MTGJSON AllPrintings, source des Cartes, Impressions, Sets, Rulings et légalités.

**Base règles**:
La base locale contenant les Règles complètes, distincte de la Base cartes.

## Decks

**Decklist**:
La représentation texte d'un Deck fournie par l'utilisateur, avec une section Commander et une section Deck ; toute autre section est ignorée.
_Avoid_: Liste, export

**Deck**:
Un deck Commander : exactement un Commandant et 99 autres cartes en un exemplaire (hors terrains de base).
_Avoid_: Build, liste

**Commandant**:
La créature légendaire (ou carte autorisée) qui définit l'identité de couleur du Deck ; un seul par Deck.
_Avoid_: Général, commander

**Identité de couleur**:
L'ensemble des couleurs d'une Carte (coût et texte compris) ; celle du Commandant borne les Cartes autorisées dans le Deck.
_Avoid_: Couleurs du deck

**Carte non résolue**:
Une ligne de Decklist dont le nom ne correspond exactement à aucune Carte.

## Analyse

**Rôle**:
La fonction d'une Carte dans un Deck (ramp, pioche, removal ciblé, wipe, protection, terrain…) ; une Carte a zéro ou plusieurs Rôles.
_Avoid_: Tag, catégorie

**Thème**:
Une stratégie transversale portée par des Cartes (tokens, compteurs +1/+1, aristocrats, tribal…) ; une Carte a zéro ou plusieurs Thèmes.
_Avoid_: Archétype, tag, mécanique

**Synergie**:
Un Thème partagé par plusieurs Cartes du même Deck.
_Avoid_: Combo, interaction

**Point faible**:
Un écart mesurable du Deck par rapport à une attente : Rôle sous-représenté, courbe de mana déséquilibrée, base de mana insuffisante, Carte illégale.
_Avoid_: Problème, défaut

**Suggestion**:
Une Carte légale en Commander, dans l'Identité de couleur du Commandant, absente du Deck, proposée pour combler un Point faible ou renforcer une Synergie.
_Avoid_: Recommandation, upgrade

**Carte à retirer**:
Une Carte du Deck qu'une Suggestion propose de remplacer ; facultative.
_Avoid_: Cut, coupe

**Verdict**:
L'appréciation d'ensemble d'un Deck, composée d'un Résumé, de Points forts, de Faiblesses et de Priorités.
_Avoid_: Conclusion, avis

**Faiblesse**:
Un défaut du Deck relevé dans le Verdict ; appréciation qualitative qui peut s'appuyer sur des Points faibles sans s'y limiter.
_Avoid_: Point faible (qui désigne l'écart mesuré)

**Priorité**:
Une action d'amélioration du Deck, ordonnée par importance dans le Verdict.
_Avoid_: Todo, conseil

**Rapport d'analyse**:
Le document HTML présentant le Verdict, les Points faibles, Synergies et Suggestions pour un Deck.
_Avoid_: Analyse, report
