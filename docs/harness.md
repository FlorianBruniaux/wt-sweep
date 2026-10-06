# Configuration IA du dépôt

[Index documentaire](README.md) · [Contribution](../CONTRIBUTING.md)

`harness/instructions.md`, `harness/agents.json`, `harness/agents/*.md`,
`harness/skills.json` et `harness/skills/` sont les sources éditables.
`scripts/sync-ai.py` génère les deux instructions racine, les agents Claude/Codex,
les skills des deux clients, la configuration Codex et un inventaire SHA-256.
Aucun fichier global ni secret n'entre dans les projections.

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

## Alignement avec le triage livré

Les instructions racine renvoient à la [feuille de route](roadmap.md), au plan
T01-T09 et au [checkpoint de validation](finish-ledger.md#triage-t01-t09).
Les quatre rôles reprennent les limites utiles à leur tâche : preuves courantes,
conditions C1-C5, priorité à la conservation, budgets et couverture des vues.
`CLAUDE.md`, `AGENTS.md` et les agents des deux clients sont produits depuis
ces sources communes.

La configuration conserve l'héritage du modèle et le plafond de trois agents.
Les modèles observés pendant l'implémentation sont consignés dans le registre ;
ils ne sont pas imposés aux sessions suivantes. Aucun changement de permission,
hook ou réglage global n'accompagne cette mise à jour.

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

Vérification du 06/10/2026 après mise à jour du triage : les 12 fichiers sont
synchronisés et les 7 tests de génération passent avec Python 3.14. Ces tests
vérifient les projections, la répétition sans changement et les refus de dérive
ou de chemin invalide dans des dossiers jetables. Le chargement natif reste
`UNKNOWN`.

## Chargement natif

Ouvrir une nouvelle session Claude ou Codex depuis la racine, avec la confiance
projet normale du client. Vérifier que les instructions et les quatre rôles sont
découverts, puis tester un rôle sur une question bornée et sans effet de bord.
Consigner séparément présence, syntaxe, découverte, invocation et effet observé.
Un contexte injecté par le testeur ne prouve pas un chargement automatique.

État du socle : projections présentes ; découverte et invocation natives UNKNOWN.
Aucun hook de session ou de réécriture n'est installé. Les contrôles sont
explicites et repris par la CI. Le skill utilisateur `wt-sweep` complète les rôles
de développement. Il sert à conduire une revue CLI et à expliquer ses résultats ; il n’exécute aucun nettoyage.

Exécution du 04/10/2026 : l'API de collaboration de la session expose les rôles
`sweep-implementer` et `sweep-test-engineer` ; les trois agents lancés avec ces
rôles ont produit les fichiers et résultats recensés dans [progress.md](progress.md).
Cette invocation est observée. Elle ne prouve ni l'origine exacte des définitions
chargées par le client, ni le chargement automatique des projections à une nouvelle
session, ni l'activation d'un hook. La phrase de statut des instructions a été
remplacée dans la source canonique par un renvoi au registre, puis régénérée.
Les modèles et les définitions des rôles restent inchangés.

## Skill utilisateur wt-sweep

Le [guide d’utilisation](assistant-skill.md) contient les invocations et les
prompts. Une source sous `harness/skills/wt-sweep` produit des copies identiques
sous `.claude/skills/wt-sweep` et `.agents/skills/wt-sweep`. Codex utilise ici
`.agents/skills`, distinct de `.codex/agents` pour les rôles de développement.

Le manifeste énumère chaque ressource. Le générateur vérifie les chemins,
le frontmatter commun (nom simple et description entre guillemets JSON), les
liens Markdown locaux et les scénarios. Les bundles emploient des liens inline
relatifs à des fichiers inclus ; les imports ou liens de référence implicites
ne font pas partie de ce format. Les liens externes ne sont pas téléchargés.
Il refuse une ressource absente, un lien sortant du bundle, un lien symbolique,
un fichier supplémentaire dans un bundle géré ou une préimage modifiée.
Les skills sans rapport avec le manifeste sont préservés.

Le corpus `evals/scenarios.json` contient des demandes positives et négatives
en français et en anglais. Il sert au routeur BM25 local lorsqu’il est installé ;
les deux clients peuvent aussi invoquer le skill explicitement sans ce routeur.
Le dépôt n’installe pas de hook pour l’activer.

Les contrôles de routage sont distincts de la CI portable. Pour les rejouer sur
une machine équipée du routeur ai-agents, depuis ce checkout :

```sh
router_root="$HOME/.config/ai-agents/current/hooks/skill-router"
for host in claude codex; do
  SKILL_ROUTER_HOST="$host" SKILL_ROUTER_CWD="$PWD" \
    node "$router_root/routing/build-index.js"
done
```

Cette commande reconstruit les caches locaux du routeur pour ce périmètre. Elle
ne modifie ni ses règles, ni ses hooks, ni les skills globaux. Vérifier ensuite
`eligible` pour `wt-sweep`, le seuil de qualité global et les résultats de
`routePayload` sur des demandes nouvelles, positives et négatives. Le détail de
l’essai observé figure dans [progress.md](progress.md#skill-wt-sweep-06102026).
La génération, le routage, l’invocation native et l’effet produit restent quatre
preuves distinctes.


Le skill inclut une [politique de modèles et de délégation](../harness/skills/wt-sweep/references/model-policy.md).
Elle sélectionne les paramètres disponibles à l’invocation d’un sous-agent,
sans modifier les modèles hérités des quatre rôles de développement ni la
configuration globale. Un réglage recommandé mais indisponible reste signalé
comme non appliqué. La concurrence de ce parcours est plafonnée à deux enfants,
sous réserve de la limite réelle du client.
