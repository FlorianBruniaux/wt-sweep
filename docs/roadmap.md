# Feuille de route

[Index documentaire](README.md) · [Preuves du triage](finish-ledger.md#triage-t01-t09) · [Plan T01-T09](superpowers/plans/2026-10-05-worktree-triage-v2.md)

## Alpha.2 : interface et distribution, 11/10/2026

[v0.1.0-alpha.2](https://github.com/FlorianBruniaux/wt-sweep/releases/tag/v0.1.0-alpha.2)
regroupe les finitions HTML et disque : bilan compact, liste ouverte, actions
conseillées, filtres rapides, guide cliquable, recherche clavier et jauges par
volume. Les détails techniques restent accessibles. Les verdicts C1-C5,
les données source et les limites APFS sont conservés.

La [PR #32](https://github.com/FlorianBruniaux/wt-sweep/pull/32) a livré le dashboard
et le rétablissement de la CI automatique pour le propriétaire. Les contrôles
navigateur couvrent aussi les deux rapports à 390 et 1440 px, en clair/sombre,
avec et sans JavaScript. La publication du site reste manuelle.

Le [guide de release](release.md) donne la recette du nouveau paquet Apple
Silicon : commit propre, CI exacte, archive et téléchargement public vérifiés.
Le [registre](progress.md) distingue validation locale et livraison distante.
L’archive alpha.1 reste inchangée.

## Évaluations conclues, 11/10/2026

Les [décisions et recettes](research/extension-decisions-2026-10-11.md) couvrent
les cinq essais identifiés. Un arrêt d’option termine son évaluation sans
annoncer une fonctionnalité livrée.

| Issue | Décision | Condition pour rouvrir |
| --- | --- | --- |
| [#20](https://github.com/FlorianBruniaux/wt-sweep/issues/20) | Garder le collecteur scalaire. Le prototype natif gagne sur les fixtures, mais son intégration produit reste non qualifiée. | Adaptation aux protections du produit et qualification des frontières système. |
| [#27](https://github.com/FlorianBruniaux/wt-sweep/issues/27) | Utiliser le dispatch `wt sweep` existant, testé sur Worktrunk v0.80.0. | Régression ou nouveau besoin d’intégration reproduit. |
| [#28](https://github.com/FlorianBruniaux/wt-sweep/issues/28) | Garder le lecteur de provenance expérimental, hors CLI. | Besoin d’afficher les déclarations avec leurs limites d’identité acceptées. |
| [#29](https://github.com/FlorianBruniaux/wt-sweep/issues/29) | E1 non démontrée ; `apply` reste bloqué. | Autorité indépendante couvrant FD, mappings, nouveaux opens et perte du garde. |
| [#30](https://github.com/FlorianBruniaux/wt-sweep/issues/30) | Garder la recette APFS expérimentale ; création confiée aux outils existants. | Besoin non couvert, avec contrat de création et preuves propres. |

L’épique [#14](https://github.com/FlorianBruniaux/wt-sweep/issues/14) regroupe ces
quatre dernières décisions. Aucune option n’active sauvegarde, restauration ou
retrait. Les tests et la livraison ont leur [registre](progress.md).

## Suite possible, non engagée

- Qualifier Intel, d’autres systèmes, la notarisation Apple ou un tap Homebrew
  si la distribution actuelle ne couvre pas un besoin reproduit.
- Reprendre les audits réels par worktree et par budget selon les lacunes du
  rapport. Les dernières mesures restent des observations datées.
- Réouvrir une extension uniquement selon les conditions ci-dessus. Aucune
  nouvelle fonction de gestion n’est nécessaire pour utiliser l’audit alpha.2.

Les sections suivantes conservent l’historique des lots et leurs plans d’origine.

## Finitions de lecture, 10/10/2026

Le lot `60f0038` ajoute le récapitulatif console centré sur le disque, la sortie
HTML sur le Bureau avec `--open`, les aides sur les intitulés et les liens de
dépôt, branche, commit et PR/MR. Les guides CLI, HTML et forge ainsi que les pages
anglaises et françaises du site décrivent ces comportements. La validation est
consignée dans [les résultats d’implémentation](progress.md#liens-vers-les-forges-10102026).

Ces finitions ne changent pas les verdicts ni les plafonds de collecte. Les liens
s’appuient sur les données locales ou importées ; ils ne confirment pas l’état
actuel de GitHub ou GitLab. L’évaluation native de #20 reste facultative après la publication de l’alpha.

## Suite de la première alpha, 10/10/2026

[v0.1.0-alpha.1](https://github.com/FlorianBruniaux/wt-sweep/releases/tag/v0.1.0-alpha.1) distribue l’audit en lecture seule sur macOS Apple Silicon. [#31](https://github.com/FlorianBruniaux/wt-sweep/issues/31) suit le commit validé, le paquet, son téléchargement et sa recette ; le [guide de release](release.md) donne les commandes reproductibles.

La licence FSL-1.1-ALv2 est conservée. La publication ne qualifie ni les autres plateformes, ni la notarisation Apple, ni la suppression de worktrees. Le site et les guides anglais/français distinguent le paquet et la compilation depuis les sources.

À la publication de la première alpha, les options suivantes restaient à évaluer.
Leurs décisions du 11 octobre sont consignées plus haut :

- [#20](https://github.com/FlorianBruniaux/wt-sweep/issues/20) : évaluation facultative du collecteur disque natif, sans promesse de gain sur le parcours complet.
- [#27](https://github.com/FlorianBruniaux/wt-sweep/issues/27) : accès facultatif depuis Worktrunk.
- [#28](https://github.com/FlorianBruniaux/wt-sweep/issues/28) : import passif de provenance des sessions.
- [#29](https://github.com/FlorianBruniaux/wt-sweep/issues/29) : démonstration d’exclusion des écritures avant toute récupération ou tout retrait.
- [#30](https://github.com/FlorianBruniaux/wt-sweep/issues/30) : expérience de création par clone APFS, seulement si le besoin est reproduit.

[#14](https://github.com/FlorianBruniaux/wt-sweep/issues/14) indexe les quatre dernières options. Elles ne bloquent pas l’alpha. Homebrew reste une distribution à évaluer après la validation du paquet ; aucune installation via un tap n’est annoncée.

### Plan initial de l’évaluation native de #20

Ce plan est conclu par la [campagne du 11 octobre](research/native-disk-evaluation-2026-10-11.md).
Les critères d’origine sont conservés ci-dessous.

Le ciblage disque, les totaux de sélection, le partage du budget et la progression
sont livrés par #22. Le travail restant concerne `getattrlistbulk`, encore absent
du collecteur produit. L’objectif est de réduire le coût de lecture des métadonnées
en conservant les mêmes observations et refus. Le lot n’augmente pas la couverture
Git et ne fournit aucune estimation de l’espace récupérable.

1. **Décodage borné.** Tester réponses tronquées, longueurs invalides, attributs
   absents et erreurs par entrée. Une entrée illisible doit produire un refus
   ou une lacune explicite ; aucune taille manquante ne devient zéro.
2. **Parcours et conservation.** Conserver la descente par descripteur, les
   identités périphérique/inode, les octets des noms, les ignorés et les plafonds.
   Tester liens, fichiers spéciaux, montages, firmlinks, dataless et changements
   concurrents. Garder une voie scalaire contrôlée ou un refus documenté.
3. **Comparaison complète.** Exécuter les deux collecteurs sur les mêmes fixtures,
   avec oracle indépendant, plusieurs tailles de dossiers et cas de refus.
   Vérifier tailles, couverture, déduplication et conservation avant de comparer
   durées et appels système. La sonde actuelle ne mesure pas ce parcours complet.
4. **Décision d’intégration.** Intégrer seulement si les garanties sont conservées
   et si la campagne montre un bénéfice sur le collecteur complet. Sinon garder
   le collecteur scalaire et clôturer l’essai avec son résultat. Mettre à jour
   le guide disque, le changelog et les preuves avant de clore #20.

Les critères détaillés restent dans [#20](https://github.com/FlorianBruniaux/wt-sweep/issues/20)
et la [recette à corpus fixe](../experiments/disk-scope/README.md). Aucun gain chiffré
n’est promis par ce plan. L’évaluation peut se conclure sans intégration native.

### Usage réel et vision

La [recette du 9 octobre](research/real-family-acceptance-2026-10-09.md) laisse
59 inscriptions à vérifier après deux reprises ciblées. Poursuivre la revue
par dossiers prioritaires ; une mesure disque complète ne termine pas leurs
preuves Git ou locales. Aucun candidat n’a été établi dans cette campagne.
Les essais facultatifs ont depuis leurs [décisions du 11 octobre](research/extension-decisions-2026-10-11.md).
Sauvegarde, restauration et retrait restent indisponibles.

## Jalon d’audit livré le 08/10/2026

[Milestone clos](https://github.com/FlorianBruniaux/wt-sweep/milestone/1) · [Épique #8 close](https://github.com/FlorianBruniaux/wt-sweep/issues/8)

La [PR #15](https://github.com/FlorianBruniaux/wt-sweep/pull/15) a livré les cinq lots au commit
`0c3e1a59e380175991fb2ab7d539d1e87e04f67e`. Les issues conservent leurs critères,
preuves et limites.

| Livrable | État |
| --- | --- |
| [#9 : console, disque et métriques](https://github.com/FlorianBruniaux/wt-sweep/issues/9) | Livré ; validation locale complète, exception CI consignée. |
| [#10 : HTML autonome par projet](https://github.com/FlorianBruniaux/wt-sweep/issues/10) | Livré ; recherche, tri, filtres et preuves capturées. |
| [#11 : collecte et premier résultat utile](https://github.com/FlorianBruniaux/wt-sweep/issues/11) | Livré ; `intent-to-add` traité, mesures bornées, aucune accélération générale démontrée. |
| [#12 : comparaison Sweep, Holt et AgentRinse](https://github.com/FlorianBruniaux/wt-sweep/issues/12) | 48 audits sur fixtures ; conserver le moteur d’audit Sweep. |
| [#13 : recette multi-projets et documentation](https://github.com/FlorianBruniaux/wt-sweep/issues/13) | Terminée sur le contenu intégré ; limites de couverture conservées. |

La [recette du 08/10](https://github.com/FlorianBruniaux/wt-sweep/issues/13#issuecomment-6052388331)
retrouve 63 inscriptions connues sur deux racines choisies : 12 `keep`,
51 `verify`, aucun candidat. Les 63 résumés locaux et 58 des 65 mesures disque
restent partiels. Le parcours atteint sa limite de répertoires ; aucune exhaustivité
machine ni estimation d’espace récupérable APFS n’est démontrée.

## Suite de la recette réelle du 08/10

La [PR #18](https://github.com/FlorianBruniaux/wt-sweep/pull/18) a livré les
finitions HTML au commit `9d69df95b1fbe6da12b6e047d114d714c08e4583`.
Les issues #16 et #17 sont closes ; la
[CI de ce SHA](https://github.com/FlorianBruniaux/wt-sweep/actions/runs/37762302133)
a réussi. La cause administrative du blocage précédent n'a pas été inspectée.

La recette multi-racines trouve 307 familles et 418 chemins de worktree avant
la limite de découverte, avec 8 366 chemins encore en attente. Le comptage
disque observe 184,42 Go alloués, partiels et sans estimation de récupération
APFS. Sur 725 racines, 45 sont complètes, une partielle et 679 non mesurées.
Deux revues ciblées conservent les worktrees examinés ; leurs preuves locales
et privées restent partielles. Les rapports privés sont conservés localement.

La [PR #22](https://github.com/FlorianBruniaux/wt-sweep/pull/22) a livré la mesure
disque ciblée et le partage de budget au commit
`d99fefd67e6a533339b5100233aed0e9f53866da`. La
[CI de main](https://github.com/FlorianBruniaux/wt-sweep/actions/runs/37797546747)
a réussi. La voie native `getattrlistbulk` reste expérimentale.

État des lots issus de ces mesures :

1. [#20 : mesure disque ciblée et répartition du budget](https://github.com/FlorianBruniaux/wt-sweep/issues/20).
   Livré par #22 : racines demandées distinctes, partage des plafonds et
   progression bornée en sortie redirigée. La voie native groupée reste une sonde,
   sans intégration produit ni promesse d'accélération générale.
2. [#21 : lectures Git groupées](https://github.com/FlorianBruniaux/wt-sweep/issues/21).
   Sessions de commits bornées, vérification des objets et revalidations courantes.
   Le [corpus fixe](research/history-batch-benchmark-2026-10-08.md) conserve les
   mêmes preuves : médiane de 21,0 à 12,1 s, avec les limites de mesure documentées.
   La [recette ciblée](progress.md#recette-réelle-ciblée) conserve un worktree
   et signale les refs modifiées pendant la collecte. Livré par #23 ; les
   validations locales et distantes sont séparées dans le registre courant.
3. [#19 : poursuite d'une découverte interrompue](https://github.com/FlorianBruniaux/wt-sweep/issues/19).
   `scan --checkpoint-out` puis `scan --resume` pour des racines explicites. Le [guide](discovery-resume.md) décrit la frontière
   conservée, les contrôles d'identité et les intervalles séparés. La reprise doit
   rejouer le répertoire interrompu et relire les métadonnées des chemins en
   attente. Le listing courant tronque avant de trier : le dernier nom trié
   ne constitue donc pas un curseur de reprise fiable. La fixture retrouve trois
   repos sans relister les dossiers terminés ; les 24 passages coûtent davantage
   qu’un scan complet en une passe. Livré par #23 ; la reprise conserve ses
   intervalles distincts et ne constitue pas un instantané.

Chaque correctif doit garder les verdicts et la conservation sur fixtures,
puis fournir une mesure à données constantes avant une nouvelle recette réelle.

## Vision initiale après le jalon du 08/10

Les options ci-dessous sont désormais évaluées ; leurs conclusions figurent
dans les [décisions du 11 octobre](research/extension-decisions-2026-10-11.md).

L'[épique de vision #14](https://github.com/FlorianBruniaux/wt-sweep/issues/14)
reste hors du milestone. Elle conserve D3 (raccordement facultatif à Worktrunk),
D4 (provenance importée), D5 (récupération et retrait) et D6 (expérience APFS).
Ces options ne sont pas des engagements d'implémentation.

Avant d'engager une extension, documenter le besoin, l'alternative existante,
l'essai et le critère d'arrêt. Si un outil couvre déjà le besoin et les garanties
recherchées, privilégier l'adoption ou un adaptateur. Sweep ne prévoit pas de
reconstruire un gestionnaire de cycle de vie, un ADE ou un registre obligatoire.
Le retrait reste soumis à E1/U5-U7.

## Référence historique du 06/10/2026

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
| Découverte U1-U4, T08 | Un repo, plusieurs racines ou les volumes visibles ; origines, progression, interruptions et lacunes conservées. Le parcours global a été testé avec des volumes injectés. |
| T01-T04 | Cibles explicites, graphe Git, état local courant, stashes partagés et historique privé, avec budgets et inconnues. |
| T05-T07 | Trois verdicts et conditions C1-C5, cibles par famille, filtres, groupes, contexte de commit, changelog et forge sans autorité sur l'intégration. |
| T09 | Commande `review`, aide par commande, inventaire original et rapports liés par un reçu. |
| Documentation | README anglais/français, tutoriel sur un repo de test, commandes pour un repo réel, prompt d'exemple, contrats et guides CLI. |

Les contrôles du socle et son ancienne CI sont détaillés dans le
[registre de livraison du 06/10](finish-ledger.md#livraison-github-du-06102026).
Pour le contenu fusionné par #15, #9 consigne les contrôles locaux et #17 le
blocage de la CI distante.
Consulter le [workflow](https://github.com/FlorianBruniaux/wt-sweep/actions/workflows/ci.yml)
pour le statut d'un commit précis. La publication des sources ne constitue pas
une release binaire.

## Rapport HTML par projet

Le [rendu HTML](html-report-plan.md) est raccordé à `scan`, `review` et `plan`
avec `--html`. Sur macOS, `--open` enregistre les rapports sur le Bureau et
demande leur ouverture ; `--run-dir BASE` choisit un autre dossier existant.
Le rapport affiche les familles, verdicts, tailles et lacunes sans modifier le jugement. Les preuves de validation et de livraison sont
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
sur des corpus de 50 et 100 worktrees. Les appels Git, les ignorés et l’historique privé restent
mesurés séparément ; la reprise persistante demeure hors de ce lot. Les
indicateurs ne lèvent pas ces limites de collecte.

## Accompagnement par un assistant

Le [skill du repo](assistant-skill.md) fournit un parcours de revue et de lecture
progressive pour Claude et Codex, avec des suggestions de conservation et de
vérification. Il explique les plafonds de collecte et peut préparer un passage ciblé
`--budget thorough` après lecture d’un run standard. Il ne débloque pas `apply`.
Les preuves de génération, de routage et d’utilisation de la CLI sont distinguées
dans le [registre courant](progress.md#skill-wt-sweep-06102026).

## Collecte des gros repos : A1-A6

Les lots A1-A5 ont leurs positifs sur fixtures : 4 200 blobs distincts, 600
commits et 32 racines privées aboutissent à un candidat avec preuves complètes.
Une famille de 50 worktrees conserve deux `keep` et établit 48 candidats. Ces
résultats et leur oracle de conservation concernent les fixtures possédées ;
ils ne certifient pas le repo réel. A6 a ensuite exécuté les passages ciblés
et la revue de la famille complète, avec les limites détaillées ci-dessous.

| Lot | État observable |
| --- | --- |
| A1 : dépasser les anciens plafonds | Fixture positive au-delà de 4 096 objets et d’un historique de 512 étapes, avec les trois verdicts conservés. |
| A2 : collecte locale | HEAD/index/disque et cinq zéros complets sur le grand positif ; fichiers non suivis conservés comme raison de `keep`. |
| A3 : historique privé | 32 racines couvertes ; les refs directement utiles évitent une expansion inutile. Un suivi borné par quantum réduit les lectures sur sa fixture. |
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
Dans cette recette historique, les phases locale et privée atteignaient leurs
plafonds. L’unique ligne `verify` gardait une ascendance, une entrée d’index
`intent-to-add` non interprétée et un historique privé incomplet. #11 a depuis
corrigé l’interprétation de cette entrée ; des zéros incomplets ne prouvent
toujours pas la propreté.
Le repo évolue entre passages : aucune comparaison réelle n’est un benchmark
contrôlé à données constantes.

La recette multi-racines suivante est consignée dans #13, avec ses racines,
inscriptions connues et lacunes. Un nouveau scan global réel exige son périmètre
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
| U5 | Sauvegarder le contrat complet et restaurer indépendamment, sans accès au repo source, après validation d'E1. |
| U6 | Retirer et restaurer des fixtures temporaires, avec fautes et interruptions injectées. |
| U7 | Revue indépendante et approbation humaine liée à la sélection exacte avant une exécution réelle. |

`apply` reste indisponible. Les conditions et les alternatives à comparer sont
consignées dans le [plan d'acceptation](plan.md) et la
[décision concernant le retrait](decisions/apply-readiness.md).
