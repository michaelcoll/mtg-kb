# MTG Knowledge Base

Base de connaissances Magic: The Gathering locale, utilisée pour interroger cartes, sets et règles, et pour analyser et améliorer des decks Commander.

## Données de référence

**Carte**:
Une carte Magic identifiée par son nom oracle unique, indépendamment de ses impressions ; porte le texte, le coût, les types, l'identité de couleur et la légalité.
_Avoid_: Card print, fiche

**Face**:
La représentation jouable d'une des deux moitiés d'une Carte multi-face (transform, modal_dfc, split, adventure, aftermath, flip). Une Carte a une ou deux Faces ; chaque Face porte son propre oracle_text/type_line/mana_cost et peut avoir ses propres Rôles.
_Avoid_: Verso, côté

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

**Thème majeur**:
Un Thème porté par au moins 8 Cartes du Deck ; distingue une stratégie réellement construite d'un Thème tribal accidentel (2-3 Cartes qui partagent un sous-type sans que ce soit voulu). Seuls les Thèmes majeurs alimentent les Candidats.
_Avoid_: Thème dominant, archétype

**Correction**:
Une valeur fixée à la main (Rôles, Thèmes ou légalité Commander) pour une Carte, qui remplace ce que `kb` détecte, avec un motif obligatoire. Elle est stockée hors de la Base cartes et s'applique partout où la Carte est lue. Rôles et Thèmes corrigés ne sont pas limités aux valeurs détectables.
_Avoid_: Override, patch, surcharge

**Synergie**:
Un Thème partagé par plusieurs Cartes du même Deck.
_Avoid_: Combo, interaction

**Point faible**:
Un écart mesurable du Deck par rapport à une attente : Rôle sous-représenté, courbe de mana déséquilibrée, base de mana insuffisante, Carte illégale.
_Avoid_: Problème, défaut

**Candidat**:
Une Carte légale en Commander, dans l'Identité de couleur du Commandant, absente du Deck, retenue par `kb analyze` sur tout le pool des Cartes légales (pas seulement un extrait alphabétique) pour correspondre à un Rôle sous-représenté ou à un Thème majeur du Deck ; au plus 10 Candidats par Rôle sous-représenté et par Thème majeur, dédupliqués. Base des Suggestions choisies par Claude.
_Avoid_: Suggestion (qui désigne le choix final, pas le calcul de `kb`), résultat de recherche

**Source externe**:
Un service tiers qui recommande des Cartes pour un Commandant ou une Decklist (EDHREC, Recommander).
_Avoid_: API, provider

**Recommandation externe**:
Une Carte proposée par une Source externe, après filtrage par `kb` (résolue, légale en Commander, dans l'Identité de couleur, absente du Deck).
_Avoid_: Suggestion, Candidat

**Origine**:
La provenance d'une Suggestion : Candidat de `kb`, Recommandation externe d'une Source, ou investigation de Claude hors de ces listes ; une Suggestion peut avoir plusieurs Origines.
_Avoid_: Source (qui désigne le service tiers)

**Suggestion**:
Une Carte légale en Commander, dans l'Identité de couleur du Commandant, absente du Deck, proposée pour combler un Point faible ou renforcer une Synergie.
_Avoid_: Recommandation (réservé à la Recommandation externe), upgrade

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
