# Configuration IA du dépôt

[Index documentaire](README.md) · [Contribution](../CONTRIBUTING.md)

`harness/instructions.md`, `harness/agents.json` et `harness/agents/*.md` sont les
sources éditables. `scripts/sync-ai.py` génère les deux instructions racine, les
agents Claude/Codex, la configuration Codex et un inventaire SHA-256. Aucun fichier
global ni secret n'entre dans les projections.

| Rôle | Périmètre |
|---|---|
| sweep-explorer | Lecture, recherche, carte des contrats |
| sweep-implementer | Fichiers produit explicitement attribués et tests |
| sweep-test-engineer | Fixtures jetables, oracle indépendant et cas adversariaux |
| sweep-reviewer | Revue en lecture seule des preuves et chemins de perte |

Les modèles héritent de la session. Claude restreint les outils déclarés des
rôles de lecture ; Codex déclare un sandbox read-only pour ces rôles. Ces fichiers
ne prouvent pas que le client les a chargés ni que l'isolation a été exercée.
Les trois créneaux de délégation sont un plafond de configuration, pas un lancement.

## Modifier et vérifier

Depuis la racine, avec Python 3.11 ou ultérieur :

```sh
python3 -B scripts/sync-ai.py --check
python3 -B -m unittest discover -s tests -p 'test_*.py' -v
# Après modification volontaire des sources, sans autre écrivain :
python3 -B scripts/sync-ai.py --write
python3 -B scripts/sync-ai.py --check
```

Le mode check ne répare rien. Le générateur refuse les chemins symboliques, les
agents inconnus dans les répertoires réservés et les sorties modifiées depuis le
dernier inventaire. Pour une sortie modifiée, préserver son diff, reporter la
modification voulue dans la source, puis réconcilier la préimage avant génération.
Un renommage de rôle exige une réconciliation explicite des anciennes sorties.

Les écritures sont coopératives, sans transaction multi-fichier ni garantie de
résistance à une coupure. Exclure les autres écrivains pendant une génération.
Après une interruption, conserver les fichiers présents et examiner la dérive ;
ne pas forcer l'écrasement. Utiliser le diff et la révision Git de base pour
reconstituer les préimages, après contrôle des modifications concurrentes.
Ce mécanisme ne doit pas être réutilisé comme protocole de sauvegarde de worktree.

## Chargement natif

Ouvrir une nouvelle session Claude ou Codex depuis la racine, avec la confiance
projet normale du client. Vérifier que les instructions et les quatre rôles sont
découverts, puis tester un rôle sur une question bornée et sans effet de bord.
Consigner séparément présence, syntaxe, découverte, invocation et effet observé.
Un contexte injecté par le testeur ne prouve pas un chargement automatique.

État du socle : projections présentes ; découverte et invocation natives UNKNOWN.
Aucun hook de session ou de réécriture n'est installé. Les contrôles sont
explicites et repris par la CI. Aucun nouveau skill n'est nécessaire à ce stade :
les workflows spécifiques restent dans les instructions et les rôles. Une future
création de skill devra inclure projections communes et évaluation de routage.

Exécution du 04/10/2026 : l'API de collaboration de la session expose les rôles
`sweep-implementer` et `sweep-test-engineer` ; les trois agents lancés avec ces
rôles ont produit les fichiers et résultats recensés dans [progress.md](progress.md).
Cette invocation est observée. Elle ne prouve ni l'origine exacte des définitions
chargées par le client, ni le chargement automatique des projections à une nouvelle
session, ni l'activation d'un hook. La phrase de statut des instructions a été
remplacée dans la source canonique par un renvoi au registre, puis régénérée.
Les modèles et les définitions des rôles restent inchangés.
