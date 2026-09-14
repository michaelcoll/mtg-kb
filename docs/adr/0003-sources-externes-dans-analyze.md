# Sources externes interrogées par kb analyze

`kb analyze` interroge par défaut deux Sources externes, EDHREC et Recommander, et expose leurs Recommandations externes dans le JSON à côté des Candidats. C'est la première dépendance réseau de l'analyse : jusqu'ici seul `kb update` appelait le réseau. `--offline` désactive les appels.

Les appels restent dans `kb` plutôt que chez Claude (WebFetch) : récupérer, résoudre et filtrer des listes est déterministe, ce qui prolonge l'ADR 0001. La « recherche de candidats » de `kb` couvre désormais aussi ces Sources. Claude juge le résultat sans le récupérer.

Choix retenus :

- **EDHREC** n'a pas d'API officielle. On utilise l'endpoint JSON non documenté `json.edhrec.com/pages/commanders/<slug>.json`, en acceptant qu'il change sans préavis. Il recommande par Commandant seul. Les réponses sont mises en cache 7 jours par Commandant, à côté des bases locales.
- **Recommander** a une API publique sans clé, qui recommande à partir de la Decklist complète. Pas de cache, puisque la réponse dépend de la liste. Ses conditions imposent un usage personnel non commercial et de citer la Source dans le Rapport d'analyse.
- **Filtrage dans `kb`** : chaque Recommandation externe doit être résolue dans la Base cartes, légale en Commander, dans l'Identité de couleur et absente du Deck. On en garde au plus 30 par Source.
- **Mode dégradé** : une Source en échec (réseau, HTTP 429, structure changée) n'interrompt pas l'analyse. L'erreur est portée dans le JSON et signalée dans le Rapport d'analyse.

On a écarté trois options :
- les appels côté Claude, non reproductibles et sans cache ;
- une commande `kb recommend` séparée, qui obligerait le skill à fusionner lui-même les JSON ;
- l'échec bloquant, qui rendrait l'analyse dépendante de la disponibilité de services tiers.
