# Feuille de route

[Index documentaire](README.md) · [Preuves du triage](finish-ledger.md#triage-t01-t09) · [Plan T01-T09](superpowers/plans/2026-10-05-worktree-triage-v2.md)

## Jalon courant : audit lisible en lecture seule

[Milestone GitHub](https://github.com/FlorianBruniaux/wt-sweep/milestone/1) · [Épique de suivi #8](https://github.com/FlorianBruniaux/wt-sweep/issues/8)

Le jalon termine le parcours d'audit : choisir des racines, retrouver les
worktrees, comprendre leur état et leur taille, puis lire un rapport HTML par
projet. Les issues portent les critères d'acceptation et l'état courant ; ce
document conserve le périmètre et les limites.

| Ordre | Livrable suivi | Dépendances |
| --- | --- | --- |
| 1 | [#9 : stabiliser et publier les ajouts console, disque et métriques](https://github.com/FlorianBruniaux/wt-sweep/issues/9) | Vérifications complètes et écart intermittent à 100 worktrees à expliquer. |
| 2 | [#10, D1 : HTML autonome par projet](https://github.com/FlorianBruniaux/wt-sweep/issues/10) | Intégration après #9 ; développement possible en parallèle de #11. |
| 3 | [#11, D2 : collecte et premier résultat utile](https://github.com/FlorianBruniaux/wt-sweep/issues/11) | Intégration après #9 ; budgets et preuves préservés. |
| 4 | [#12 : comparaison Sweep, Holt et AgentRinse sur fixtures](https://github.com/FlorianBruniaux/wt-sweep/issues/12) | Indépendante du HTML ; décision requise avant une extension de gestion. |
| 5 | [#13 : recette multi-projets et documentation finale](https://github.com/FlorianBruniaux/wt-sweep/issues/13) | Après #9, #10 et #11 ; décision de #12 consignée. |

Le jalon se clôt avec les preuves des cinq tâches, une recette sur le SHA livré
et des rapports texte, JSON et HTML concordants. Les inconnues justifiées restent
visibles. La recette réelle porte sur des racines explicitement choisies ; la
création des issues ne déclenche aucun scan.

## Vision après ce jalon

L'[épique de vision #14](https://github.com/FlorianBruniaux/wt-sweep/issues/14)
reste hors du milestone. Elle conserve D3 (raccordement facultatif à Worktrunk),
D4 (provenance importée), D5 (récupération et retrait) et D6 (expérience APFS).
Ces options ne sont pas des engagements d'implémentation.

Avant d'engager une extension, documenter le besoin, l'alternative existante,
l'essai et le critère d'arrêt. Si un outil couvre déjà le besoin et les garanties
recherchées, privilégier l'adoption ou un adaptateur. Sweep ne prévoit pas de
reconstruire un gestionnaire de cycle de vie, un ADE ou un registre obligatoire.
Le retrait reste soumis à E1/U5-U7.

## État de référence

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

## Rapport HTML par projet

Le [rendu HTML](html-report-plan.md) est raccordé à `scan`, `review` et `plan`
avec `--html --run-dir`. Il affiche les familles, verdicts, tailles et lacunes
sans modifier le jugement. Les preuves de validation et de livraison sont
suivies dans #10 et #13 ; une implémentation locale ne vaut pas livraison distante.

## Espace disque

L’option `--disk-usage` de `scan` et `review` collecte les
tailles par worktree et administration Git, avec un total dédupliqué par inode
et des mesures partielles explicites. Les tailles sont intégrées au bilan console, au rapport
texte et au JSON ; `plan` conserve les observations capturées.
Le [guide disque](disk-usage.md) donne la commande et les limites. L’estimation
d’espace réellement récupérable sur APFS reste hors de ce comptage.

## Mesure de performance

La CLI consigne le débit de découverte et les durées monotones. Un benchmark
Rust facultatif vérifie les résultats constants. La [recette de mesure](collection-performance.md#benchmark-rust-reproductible)
compare 1 et 4 workers ; elle ne promet aucune accélération sur d’autres disques.
Le lot #11 traite `intent-to-add` et [mesure le premier fait utile](research/collection-benchmark-2026-10-07.md)
sur des corpus de 50 et 100 worktrees. Les appels Git, les ignorés et l’histoire privée restent
mesurés séparément ; la reprise persistante demeure hors de ce lot. Les
indicateurs ne lèvent pas ces limites de collecte.

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
L’entrée `intent-to-add` de cette recette historique restait inconnue même
avec le format v2. Le lot #11 traite désormais ce cas sur des fixtures SHA-1 et
SHA-256 ; seul un nouveau passage peut établir l’état actuel du worktree réel.

## Extension vers la gestion : comparaison préalable

La [lecture des 12 clones](worktree-source-analysis-2026-10-07.md) est terminée.
Elle propose D1 (HTML par projet), D2 (premier résultat utile), D3 (`wt sweep`)
et D4 (provenance importée passivement). D1/D2 appartiennent au jalon courant ; D3/D4 restent dans la vision.
Les comparaisons de récupération D5 et de création APFS D6 sont séparées.
Les 48 essais d’audit sur fixtures et la décision par capacité sont consignés
dans le [rapport de comparaison](research/worktree-runtime-comparison-2026-10-07.md) et #12.
Ils ne prouvent ni la restauration ni une équivalence de conservation générale.

La [recherche du 07/10](worktree-ecosystem-2026-10-07.md) identifie un
recouvrement avec Holt et AgentRinse pour l'audit et la récupération, et avec
Worktrunk, kwt, gwq et Workroot pour le cycle de vie. Le rapport HTML reste un
besoin de restitution ; il n'est pas une différenciation démontrée.

La comparaison d’audit retient Sweep pour ses preuves explicites et reprend des
idées de restitution. Elle ne démontre pas une équivalence de récupération.
Avant d’ajouter création, registre propriétaire ou orchestration, comparer
encore les mécanismes de récupération de Holt et AgentRinse, puis Worktrunk et
kwt pour les entrées JSON et la gestion des sessions. Les essais de cycle de vie restent hors du jalon courant. Aucun de ces projets n'est une dépendance de Sweep, et cette recherche
ne lève pas les conditions E1/U5-U7.

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
