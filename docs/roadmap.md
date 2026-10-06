# Feuille de route

[Index documentaire](README.md) · [Preuves du triage](finish-ledger.md#triage-t01-t09) · [Plan T01-T09](superpowers/plans/2026-10-05-worktree-triage-v2.md)

État du 06/10/2026 : T01 à T09 sont fusionnés dans `main` par la
[PR #1](https://github.com/FlorianBruniaux/wt-sweep/pull/1), commit
`ddda77ddba24566328a83dad79b74d943cecd8e5`. Le triage en lecture seule est livré.
La recette A6 de la famille choisie est terminée : 50 worktrees, 49 `keep`, aucun
candidat et un `verify`, avec artefacts vérifiés. Dix résumés locaux sont complets
et 40 restent partiels ; les 49 raisons de conserver ne signifient pas 49
analyses exhaustives. Le triage ne propose aucun retrait de cette famille.
Les plafonds et l’index de la ligne restante gardent des preuves inconnues.

## Livré

| Périmètre | Comportement disponible |
| --- | --- |
| Découverte U1-U4, T08 | Un dépôt, plusieurs racines ou les volumes visibles ; origines, progression, interruptions et lacunes conservées. Le parcours global a été testé avec des volumes injectés. |
| T01-T04 | Cibles explicites, graphe Git, état local courant, stashes partagés et histoire privée, avec budgets et inconnues. |
| T05-T07 | Trois verdicts et conditions C1-C5, cibles par famille, filtres, groupes, contexte de commit, changelog et forge sans autorité sur l'intégration. |
| T09 | Commande `review`, aide par commande, inventaire original et rapports liés par un reçu. |
| Documentation | README anglais/français, tutoriel jetable, commandes pour un dépôt réel, prompt d'exemple, contrats et guides CLI. |

Les contrôles locaux et la CI réussie avant fusion sont détaillés dans le
[registre de livraison](finish-ledger.md#livraison-github-du-06102026).
Consulter le [workflow](https://github.com/FlorianBruniaux/wt-sweep/actions/workflows/ci.yml)
pour le statut d'un commit précis. La publication des sources ne constitue pas
une release binaire.

## Accompagnement par un assistant

Le [skill du dépôt](assistant-skill.md) fournit un parcours de revue et de lecture
progressive pour Claude et Codex, avec des suggestions de conservation et de
vérification. Il explique les plafonds de collecte et peut préparer un passage ciblé
`--budget thorough` après lecture d’un run standard. Il ne débloque pas `apply`.
Les preuves de génération, de routage et d’utilisation de la CLI sont distinguées
dans le [registre courant](progress.md#skill-wt-sweep-06102026).

## Collecte des gros dépôts : A1-A6

Les lots A1-A5 ont leurs positifs sur fixtures : 4 200 blobs distincts, 600
commits et 32 racines privées aboutissent à un candidat avec preuves complètes.
Une famille de 50 worktrees conserve deux `keep` et établit 48 candidats. Ces
résultats et leur oracle de conservation concernent les fixtures possédées ;
ils ne certifient pas le dépôt réel. A6 a ensuite exécuté les passages ciblés
et la revue de la famille complète, avec les limites détaillées ci-dessous.

| Lot | État observable |
| --- | --- |
| A1 : dépasser les anciens plafonds | Fixture positive au-delà de 4 096 objets et d’une histoire de 512 étapes, avec les trois verdicts conservés. |
| A2 : collecte locale | HEAD/index/disque et cinq zéros complets sur le grand positif ; fichiers non suivis conservés comme raison de `keep`. |
| A3 : histoire privée | 32 racines couvertes ; les refs directement utiles évitent une expansion inutile. Un suivi borné par quantum réduit les lectures sur sa fixture. |
| A4 : profils explicites | `standard|thorough` exposés sur `review` et `plan`, avec plafonds finis et métriques de phase. |
| A5 : famille de 50 | 48 candidats simples et deux `keep`, inventaire et conservation inchangés. |
| A6 : recette réelle | Terminée en lecture seule : 50 lignes, 49 `keep`, aucun candidat, un `verify` ; dix résumés locaux complets et 40 partiels. Artefacts vérifiés, plafonds et conditions manquantes conservés. |

Commencer par standard, inspecter les faits sauvés, puis choisir un worktree
exact pour thorough avant une nouvelle revue de toute la famille. Les plafonds
ne mesurent pas le débit ; les métriques ne remplacent pas C1-C5. La priorité des
ancres privées et le transport Git ont leurs régressions ciblées. La sonde
constante de transport passe de 46 540 à 500 ms pour 64 MiB et 16 enfants ; le
second réel ciblé examine 52 213 chemins et retient le worktree. Son état local
reste incomplet au plafond d’octets. Ces mesures ne prouvent pas la complétude
de toute la famille ni une accélération d’énumération. Le [checkpoint de collecte](collection-performance.md#checkpoint-a1-a6--fixtures-et-recette-réelle)
sépare les mesures synthétiques, les passages ciblés et le bilan de famille.
Les phases locale et privée atteignent leurs plafonds. L’unique ligne `verify`
garde une ascendance mais une entrée d’index `intent-to-add` non interprétée et
une histoire privée incomplète ; ses zéros incomplets ne prouvent pas la propreté.
Le dépôt évolue entre passages : aucune comparaison réelle n’est un benchmark
contrôlé à données constantes.

Une découverte multi-racines réelle demeure une exécution distincte : choisir
les racines, comparer les familles connues, vérifier doublons, inscriptions
extérieures et zones inaccessibles. Le scan global réel exige son périmètre
explicitement demandé.

X02 (lectures Git groupées) est désormais utilisé par le résumé local : les
fixtures à 1 024 contenus distincts ou dossiers distincts reproduisent le coût
que la fixture initiale à contenu partagé ne montrait pas. Les extensions X01
(reprise) et X03 (autre format d’index) du [plan T01-T09](superpowers/plans/2026-10-05-worktree-triage-v2.md)
restent conditionnelles. La découverte réelle s’est achevée et l’index v2 est
pris en charge. Une collecte ciblée est un nouveau run, pas une reprise X01.
Le support d’un autre format d’index demande un cas concret et ses fixtures.
L’entrée `intent-to-add` du dernier run garde ses sémantiques inconnues même
avec le format v2 ; elle demande son propre contrôle.

## Nettoyage automatique : chantier séparé

| Étape | Condition de sortie |
| --- | --- |
| E1 | Démontrer l'exclusion des écritures concurrentes, y compris depuis un descripteur déjà ouvert. Les expériences actuelles ne la démontrent pas. |
| U5 | Sauvegarder le contrat complet et restaurer indépendamment, sans accès au dépôt source, après validation d'E1. |
| U6 | Retirer et restaurer des fixtures jetables, avec fautes et interruptions injectées. |
| U7 | Revue indépendante et approbation humaine liée à la sélection exacte avant une exécution réelle. |

`apply` reste indisponible. Les conditions et les alternatives à comparer sont
consignées dans le [plan d'acceptation](plan.md) et la
[décision concernant le retrait](decisions/apply-readiness.md).
