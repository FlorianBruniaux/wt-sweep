# Résultats d'implémentation

[Index documentaire](README.md) · [Contrats des artefacts](contracts.md) · [Feuille de route](roadmap.md)

L’[état publié](#état-publié-et-suite-au-09102026) sépare livraison, CI et recette
réelle. Le [lot de collecte](#sessions-git-et-reprise-de-découverte-08102026)
conserve les tests du code livré ; les sections plus anciennes sont des observations
datées, pas une description des tâches encore ouvertes.

## Liens vers les forges, 10/10/2026

`review` et `plan` capturent les remotes reconnus et les refs locales de suivi
pendant la collecte de contexte. Les rapports HTML et texte proposent des liens
vers le dépôt, la branche et le commit GitHub/GitLab. Le HTML garde les liens de
remotes dans un bloc replié et laisse le lien PR/MR directement visible. Titre,
état capturé et provenance de la demande restent consultables dans ses détails.

Le collecteur ne fait aucun appel réseau. L’égalité entre HEAD et une ref locale
de suivi ne prouve pas un push actuel. Les PR/MR exigent toujours une association
exacte avec un import `--forge`. Les instances auto-hébergées exigent cet import
pour identifier leur fournisseur ; les forks GitLab restent limités par les
identités disponibles dans l’import. `scan` ne collecte pas ces métadonnées.
`--no-context` les omet sans retirer les PR/MR déjà associées. Les verdicts de
triage ne dépendent pas de ces liens.

Les URL sont reconstruites sans identifiants, puis validées au rendu. Les noms
de branches sont encodés ; les libellés sont échappés. Une ambiguïté ou une limite
de collecte reste explicite. L’ajout facultatif au JSON conserve la sérialisation
et les empreintes des anciens rapports sans ce champ. Une copie de l’ancien
rapport de 67 worktrees a été rendue depuis ses données validées, sans collecte.

Validation locale : la suite Rust complète a passé 419 tests avec 3 ignorés.
Après la dernière correction de cadrage des clés de remote, les 6 tests du module
URL, 14 tests HTML et 3 tests de présentation du contexte passent. Formatage,
Clippy, builds debug/release et synchronisation des 22 fichiers d’instructions
passent.

La passe Python complète exécute 316 cas : 309 réussis, 6 ignorés et une erreur
dans le nouveau test d’upstream. Ce test utilisait un attribut `detached` absent
de sa fixture. Il crée désormais un vrai worktree détaché, inclus dans le
manifeste de conservation. Les 5 tests du module passent après cette correction,
sans modification du binaire. La suite complète n’a pas été rejouée après cette
correction limitée au test. Aucun autre échec n’est apparu.

Les 8 cas Chromium passent : clair/sombre, 1440/390 px, avec/sans JavaScript.
Ils vérifient le repli natif, la destination du lien PR d’un fork, l’absence de
débordement, de chargement réseau et d’accès à la fenêtre d’origine. La requête
issue du clic est interceptée avant le réseau. Safari, Firefox et la disponibilité
des pages sur les serveurs ne sont pas vérifiés. Le rapport d’exemple utilise
uniquement une famille Git jetable et une PR synthétique.

README anglais/français, aide CLI, contrats, guides forge/HTML/assistant et
changelog sont mis à jour. Les changements restent locaux ; aucun commit,
push ou contrôle de CI distante n’est effectué pour ce lot.

## Aides de navigation sur les intitulés, 10/10/2026

Les quatre boutons « ? » sont retirés du menu HTML. Les intitulés conservent
leur ancre et leur description accessible ; survol, focus et clic affichent
l’aide. Sans JavaScript, l’attribut `title` conserve l’explication native.
Le générateur, la documentation HTML et le changelog sont mis à jour.

Les 14 tests Rust de rendu HTML et les 10 tests CLI HTML passent, ainsi que
formatage, Clippy et contrôle de synchronisation des instructions. Le test
navigateur passe sur huit rendus Chromium : clair/sombre, 1440/390 px, avec
et sans JavaScript. Un premier essai a expiré pendant la navigation animée ;
le test utilise ensuite la préférence de mouvement réduit pour vérifier les
ancres sans animation. Le rendu reste configuré selon la préférence du lecteur.
Le test sans JavaScript vérifie l’attribut d’aide, pas son affichage natif.

Une copie du rapport de 67 worktrees est régénérée depuis ses JSON validés,
sans collecte. Les empreintes des cinq fichiers du run source sont inchangées.
Ces changements sont locaux ; aucun commit ni push n’est effectué pour ce lot.

## Destination et ouverture du HTML, 10/10/2026

`scan`, `review` et `plan` acceptent `--html` sans `--run-dir` sur macOS :
la sortie devient `~/Desktop/Worktree Sweep/run-*/`. Le Bureau doit exister ;
Sweep crée seulement le conteneur final, avec accès privé. Un `--run-dir`
explicite garde sa priorité et doit désigner un dossier existant. Les refus
pour cible protégée, lien symbolique ou écrasement restent appliqués.

`--open` implique le HTML. L’appel natif à `/usr/bin/open` suit la publication
réussie et son attente est bornée à cinq secondes. Un échec d’ouverture laisse
le rapport enregistré et affiche un avertissement avec son chemin. La console
propose aussi une commande de réouverture correctement citée pour le shell.
Les flux JSON et les commandes sans HTML conservent leur comportement.

Les tests ajoutés ont d’abord échoué sur le défaut de destination et l’absence
de `--open`. Les 10 tests CLI HTML passent ensuite sur des familles jetables :
destination par défaut ou explicite, exécutions répétées, reçus et empreintes,
absence d’écriture dans les sources et JSON sur stdout. Les tests du lanceur
utilisent un exécutable temporaire pour vérifier arguments, code de sortie et
timeout ; ceux de publication vérifient l’ordre de l’ouverture et son absence
après un refus. Ils ne prouvent pas à eux seuls un rendu dans un navigateur.
Les aides générale, scan, review et plan sont relues depuis le binaire release.

Le build release, le formatage et Clippy passent. La suite Rust complète réussit
avec 413 tests et 3 ignorés. Un essai natif avec trois worktrees synthétiques
publie sur le Bureau, conserve les sources et vérifie les empreintes du reçu :
2 `keep`, 1 `cleanup_candidate`, 0 `verify`. Le lanceur macOS retourne un succès ;
le rendu de la page dans l’application ouverte n’a pas été inspecté.

La suite Python complète termine avec 311 cas : 305 réussis et 6 ignorés.
Le contrôle `scripts/sync-ai.py --check` passe. Les empreintes des sources et
tests sont inchangées entre le début et la fin de cette validation. Les
modifications restent locales sur `codex/actionable-cli-recap` ; aucune CI
distante ni publication GitHub n’a été exécutée pour ce lot.

## Récap CLI orienté espace disque, 10/10/2026

Le récap `--summary` reste en anglais et commence par l’allocation observée en
unités décimales KB/MB/GB. Il distingue les relations entre commits des verdicts
C1-C5, affiche jusqu’à cinq worktrees sélectionnés par taille observée, leurs
raisons de conservation ou contrôles manquants, puis les limites atteintes.
Le total disque garde son périmètre source. Les tailles partielles ne deviennent
ni tailles complètes ni estimations d’espace récupérable.

La suggestion de reprise utilise le binaire courant et uniquement les cibles
enregistrées pour la ligne choisie. Les arguments sont cités pour le shell ;
chemins non UTF-8, caractères de contrôle et cibles absentes ou omises empêchent
la suggestion. Le produit ne lance pas cette commande. Aucun plafond de collecte,
schéma JSON ou verdict n’est modifié par ce lot.

Trois nouveaux tests de présentation ont échoué avant l’implémentation, puis
réussi. Ils couvrent le décompte par worktree malgré plusieurs cibles,
l’exclusion des résultats provisoires, les tailles absentes/partielles, le
périmètre des lignes et les arguments shell. Les 18 tests de présentation
réussissent. Les 9 tests de terminal passent, dont une commande suggérée
réellement exécutée sur un dépôt jetable, sans modification de son manifeste.

Le formatage, Clippy sur toutes les cibles et les builds debug/release passent.
La première suite Rust complète rencontre un échec dans
`old_reflog_root_covered_only_by_lightweight_tag_can_qualify_linked` : `verify`
au lieu de `cleanup_candidate`. Le test passe seul, puis la seconde suite complète
passe avec **408 tests réussis et 3 ignorés**, sans modification du test ni du
collecteur entre les deux passes. La cause de l’échec initial reste indéterminée.

La passe Python complète exécute 306 cas : 299 réussissent, 6 sont ignorés et
un échoue car il attend encore le titre `COVERAGE`. Après remplacement de
cette assertion par les sections du nouveau récap, les 14 tests du module
`test_enriched.py` passent. La suite Python complète n’est pas rejouée après
cette correction de test ; le code produit est inchangé. Les projections
Claude/Codex et `git diff --check` passent.

Une projection du rapport réel déjà sauvegardé expose 51,87 Go observés,
6 HEAD égaux ou ancêtres des cibles et les mêmes verdicts 18 / 49 / 0.
C’est une relecture de faits existants, pas un nouveau scan ni un gain de collecte.

## Préparation de la première alpha, 09/10/2026

La version préparée est `0.1.0-alpha.1` ; le [guide de publication](release.md)
décrit le périmètre et les conditions avant le tag. Cette section ne déclare
ni release publiée, ni dépôt public. Le choix de licence reste à confirmer :
la FSL conservée à ce stade ne permet pas une annonce open source immédiate.

La course signalée plus bas est reproduite par un test qui attend la sortie du
processus avant de finaliser la session. Avant correction, une sortie réussie
produit à tort `input_io`. Le correctif laisse la validation du statut de sortie
et du protocole décider lorsque le canal est déjà fermé : succès pour une sortie
0, `git_exit` pour une sortie 7. Les sept tests de session ont passé vingt
répétitions ; la suite finale ci-dessous inclut aussi ce correctif.

| Contrôle local | Résultat observé |
| --- | --- |
| Rust 1.95.0, formatage et Clippy sur toutes les cibles | Réussis, aucune tolérance de lint ajoutée |
| Tests Rust | 405 réussis, 3 ignorés |
| Python 3.12.14, tests produit | 299 réussis, 6 ignorés sur 305 cas |
| Expériences d’exclusion et harness de comparaison | Réussis |
| Build debug et release verrouillés, hors ligne | Réussis |
| Projections Claude/Codex | 22 fichiers synchronisés ; activation native non vérifiée |

Les empreintes de tous les fichiers source sont restées identiques pendant la
passe finale. L’alignement du minimum Rust sur la version testée active des
suggestions Clippy supplémentaires ; leurs transformations de conditions et de
tests de parité sont incluses dans cette validation complète.

Le contrôle Gitleaks 8.30.1 a couvert l’ascendance de main, les 13 têtes distantes
observées, les fichiers suivis avec les nouveaux documents, et les corps et
commentaires des issues/PR accessibles. Son seul signal est un faux positif
examiné : deux références Git synthétiques dans `tests/test_scale.py`. Aucun
autre secret n’a été détecté dans ce périmètre. Ce contrôle par motifs ne couvre
pas les anciens logs Actions ou les artefacts expirés.

La [CI de main sur `117f9b5`](https://github.com/FlorianBruniaux/wt-sweep/actions/runs/37917280210)
n’a démarré aucune étape : GitHub signale le paiement ou le plafond de dépense
du compte. Les résultats locaux ne remplacent pas une CI sur le commit à taguer.
La facturation n’a pas été modifiée.

### Recette de l’archive Apple Silicon

Une compilation `cargo build --release --locked --offline` depuis un clone
local neuf et propre du commit `6446ee6bc06581a840fcc8108fe6582699448074`
produit le binaire `wt-sweep 0.1.0-alpha.1`. La recette est exécutée sur macOS
26.5.1, arm64 ; elle ne qualifie pas une version minimale de macOS.

L’archive locale `wt-sweep-v0.1.0-alpha.1-aarch64-apple-darwin.tar.gz` fait
1 619 016 octets. Son SHA-256 est :

```text
b0712ea3ce18ef8580586406c591d5b48daf87026026f9c2fb16069d3d7b9c03
```

Elle contient le binaire, la licence FSL conservée, les notices des 30 dépendances
verrouillées, les licences Rust et un manifeste du commit et des fichiers.
Après extraction dans un autre dossier, les 79 fichiers du manifeste sont
vérifiés, puis `--version`, `--help` et `review --help` réussissent. La démo de
[getting-started.md](getting-started.md#disposable-demo), avec seulement le chemin
du binaire remplacé, donne **2 `keep`, 1 `cleanup_candidate`, 0 `verify`**.
Les quatre artefacts produits correspondent au reçu, y compris le HTML.

Ce paquet est un candidat local, pas un téléchargement publié. L’installation
de Rust sur une machine vierge, Gatekeeper après téléchargement, la notarisation,
Intel/Linux, le rendu navigateur et la reconnaissance native du skill ne sont
pas vérifiés par cette recette. Avant publication, reconstruire et vérifier
l’archive depuis le commit exact à taguer, avec la licence finalement retenue.

## État publié et suite au 09/10/2026

| Livraison | Commit sur main | État distant observé |
| --- | --- | --- |
| [#18 : finitions HTML](https://github.com/FlorianBruniaux/wt-sweep/pull/18) | `9d69df95b1fbe6da12b6e047d114d714c08e4583` | [CI réussie](https://github.com/FlorianBruniaux/wt-sweep/actions/runs/37762302133) ; #16 et #17 closes. |
| [#22 : disque ciblé et partage du budget](https://github.com/FlorianBruniaux/wt-sweep/pull/22) | `d99fefd67e6a533339b5100233aed0e9f53866da` | [CI réussie](https://github.com/FlorianBruniaux/wt-sweep/actions/runs/37797546747). |
| [#23 : lectures Git groupées et reprise](https://github.com/FlorianBruniaux/wt-sweep/pull/23) | `bc054af0c75ae3299fa69e094ca9d6d33e813db2` | Fusion le 08/10 à 22:02 UTC ; #19 et #21 closes. |

Les deux contrôles avant fusion de #23 ont réussi :
[run 37848305297](https://github.com/FlorianBruniaux/wt-sweep/actions/runs/37848305297)
et [run 37848280607](https://github.com/FlorianBruniaux/wt-sweep/actions/runs/37848280607).
Le [run de main 37850898399](https://github.com/FlorianBruniaux/wt-sweep/actions/runs/37850898399)
est en échec avant le démarrage du job. L’annotation GitHub, relue le 09/10,
signale le paiement du compte ou son plafond de dépense. Aucun test du code
n’a été exécuté par ce job. Une correction côté compte et une relance restent
nécessaires ; la clôture antérieure de #17 ne valide pas ce nouveau run.

La [recette locale du 9 octobre](research/real-family-acceptance-2026-10-09.md)
utilise un binaire figé dont l’arbre source est identique à ce main. Elle couvre
71 inscriptions dans une famille, avec 18 artefacts vérifiés. La revue initiale
donne 10 `keep`, 61 `verify`, aucun candidat ; deux reprises donnent `keep`.
La synthèse des derniers verdicts par chemin est 12 / 59 / 0, sans constituer
un nouveau verdict global ni un instantané. Cinq tailles prioritaires sont
complètes ; la mesure familiale de 53,64 Go reste partielle.

La reprise de découverte sur deux racines choisies retrouve les mêmes familles,
HEAD et identités qu’un scan neuf, sur deux intervalles. Douze artefacts de cette
campagne préalable ont aussi été vérifiés. Les fichiers HTML correspondent à
leurs reçus ; leur rendu navigateur n’a pas été revérifié lors de ces campagnes.
Aucun worktree ni cache n’a été supprimé.

Le prochain lot est l’évaluation du collecteur natif de [#20](https://github.com/FlorianBruniaux/wt-sweep/issues/20),
avec les [critères d’arrêt et de validation](roadmap.md#prochain-lot--terminer-lévaluation-native-de-20).
La vision #14 reste séparée. Aucun changement des verdicts, d’activation de
`apply`, de configuration Claude/Codex ou de hook n’est inclus dans cette
synchronisation documentaire.

### Vérification de la mise à jour documentaire

La synchronisation documentaire ne change ni le code Rust, ni les tests, ni
les contrats ou projections du harness. La première suite Rust a échoué sur
`git::session::tests::provisional_payload_is_refused_when_child_exits_unsuccessfully` :
`input_io` reçu au lieu de `git_exit`. La relance isolée puis la suite complète
passent sans correction : 404 tests réussis, trois ignorés. L’échec initial
reste enregistré ; ces relances ne démontrent pas sa stabilisation.

Le code refuse les preuves dans les deux cas observés. Lors de cette vérification, la fermeture du canal avant
la lecture du statut de sortie restait une hypothèse issue du code. Le test
forcé et le correctif décrits dans la préparation de l’alpha ci-dessus
reproduisent et traitent désormais cet ordre de fermeture.

La première suite Python a utilisé par erreur Python 3.9.6, inférieur au
prérequis 3.11 : `hashlib.file_digest` et `tomllib` y manquent. Ce passage ne
valide pas le produit. Avec Python 3.12.14, les 305 cas se terminent : 299 réussis,
six ignorés. Les commandes du tutoriel et du guide de contribution vérifient
désormais la version avant les suites ; le contrôle refuse 3.9.6 et accepte 3.12.14.

Formatage, Clippy, build debug et cohérence des 22 fichiers générés passent.
Les 341 liens locaux et les 22 blocs shell des 14 documents modifiés sont
vérifiés ; les 30 artefacts cités correspondent aux neuf reçus des campagnes.
Ces vérifications de syntaxe et de fichiers ne sont pas une nouvelle exécution
des tutoriels ou un contrôle visuel des captures. Les illustrations datées sont
conservées et leur provenance est précisée. L’activation native du harness
reste non vérifiée.

## Sessions Git et reprise de découverte, 08/10/2026

Le lot #21 groupe les lectures de commits dans des sessions bornées, avec
vérification des OID et validation du lot après sortie réussie de Git. Les lots
interrompus ne publient pas leurs preuves provisoires. Le lot #19 ajoute
`scan --checkpoint-out`, puis `scan --resume`, pour des racines explicites.
Les identités enregistrées sont revalidées, les chemins en attente relus et les
familles Git collectées à nouveau. Les répertoires terminés restent historiques ;
les intervalles séparés empêchent de présenter la reprise comme un instantané.

La [campagne sur corpus fixe](research/history-batch-benchmark-2026-10-08.md)
conserve les mêmes preuves sur huit passages. Pour 145 commits, les processus
passent de 146 à 39 en histoire et de 180 à 45 en histoire privée. La médiane
complète passe de 21 032 à 12 146 ms. La charge de l'hôte et les trois paires
mesurées limitent la portée de ces temps. Les plafonds des phases sont inchangés ;
le transport des sessions utilise désormais un plafond agrégé documenté,
avec 8 Mio maximum par commit et 10 secondes par processus.

La fixture de reprise retrouve trois dépôts en 24 passages : 48 listings et
122 entrées, identiques au scan neuf. Les préflights Git répétés s’accompagnent d’un
coût supérieur sur ce petit corpus : 36 174 ms cumulées contre 1 475 ms en une
passe. Ce résultat mesure une capacité de continuation sans relister les dossiers
terminés, pas une accélération face à une découverte qui peut déjà terminer.

### Recette réelle ciblée

Une revue release `thorough` sélectionne un worktree sur 67 inscriptions de sa
famille, sans scan global ni nouvelle mesure disque. Le journal local
`execution.json` du lanceur mesure 230,540 s au chronomètre monotone. La commande
termine avec le code 0 et `keep` pour au moins 7 994 fichiers ignorés observés. Les quatre
artefacts texte, HTML, inventaire et revue correspondent aux tailles et SHA-256
du reçu `partial`. Le rapport et les chemins privés restent locaux.

| Phase | Temps enregistré | Travail |
| --- | ---: | --- |
| Contrôles d'identité | 3 939 ms | Contrôles de la famille conservés |
| Histoire | 62 679 ms | 5 004 objets, 1 261 processus, 9 162 640 octets |
| Fichiers locaux | 60 473 ms | 20 500 chemins ; plafond cumulé d'octets atteint |
| Histoire privée | 97 279 ms | 25 518 objets, 1 056 processus, 21 688 747 octets |
| Revalidations | 4 369 ms | Histoire et racines privées recontrôlées |

La cible a changé pendant l'analyse : la relation observée `not_ancestor` est
rendue `unknown` avec le motif de changement. Une ref privée a aussi changé ;
23 racines restent couvertes dans le rapport et trois inconnues, avec couverture
des ancres incomplète. C2, C3, C4 et C5 restent manquantes. Le `keep` repose sur
une raison positive de conservation et ne certifie pas une revue exhaustive.

Ce dépôt est actif. Une partie de la suite Rust s'exécutait aussi pendant la fin
de la recette ; ces durées réelles ne sont pas comparées causalement aux anciens
runs. La comparaison de performance ci-dessus a été exécutée séparément sur son
corpus fixe. Aucun changement des règles de verdict ni activation de `apply`.

### Validation du lot

Le test de séparateur de payload erroné échoue d'abord avec une clôture Git
acceptée, puis passe après mémorisation de l'erreur de framing. Le test de racine
manquante conserve une preuve positive indépendante ; les deux cas sont distincts.
La revue indépendante ne relève plus de constat dans les deltas examinés.
Validation finale : formatage et Clippy passent ; 404 tests Rust réussissent,
trois sont ignorés. La suite Python exécute 305 tests : 299 réussissent et six
sont ignorés. Les six tests d'exclusion et les deux tests du harness de
comparaison passent aussi. Les binaires debug et release ainsi que les 35
empreintes du manifeste source correspondent à la campagne de mesure.
Publication confirmée par #23 ; les contrôles distants sont consignés dans
l’[état publié](#état-publié-et-suite-au-09102026).

Les README anglais et français, référence CLI, tutoriel, contrats, guides de
performance et skill sont alignés. Les 22 fichiers générés sont cohérents. Dans
un cache de routage isolé, les projections Claude et Codex sélectionnent le skill
sur 16 exemples positifs et aucun des six contre-exemples. Cela ne prouve pas
l'activation native dans un client. Les chemins Unicode et contenant un saut de
ligne sont testés octet par octet ; la création native d'un nom non UTF-8 est
refusée par le système de fichiers de cet hôte et reste non vérifiée ici.

## Mesure disque ciblée, 08/10/2026

Le lot [#20](https://github.com/FlorianBruniaux/wt-sweep/issues/20) ajoute
`--disk-root` à `scan` et `review`, avec sélection exacte de racines enregistrées.
Les lignes hors sélection restent présentes, sans mesure ni fausse lacune de
collecte. Les rapports distinguent demandé, mesuré et non demandé ; une mesure
complète de la sélection ne rend pas le total source complet. Les anciens JSON
sans sélection conservent leur digest canonique.

Les budgets globaux d'entrées et de temps sont répartis entre les racines encore
à visiter. La fixture avec un gros premier dossier vérifie que le petit dernier
dossier est mesuré complètement, que le premier reste partiel et que le plafond
global d'entrées tient. Elle ne simule pas des appels système lents pour prouver
l'équité temporelle. La progression périodique est indépendante des fins de
racine et fonctionne aussi lorsque la sortie est redirigée. Un test reproduit
le report de la cadence trouvé en revue, puis passe avec sa correction.

La [comparaison reproductible](../experiments/disk-scope/README.md) couvre trois
corpus fixes, avec un oracle de tailles indépendant et conservation des sources.
Les médianes disque ancien/nouveau sont 1/1 ms pour 174 entrées, 2/2 ms pour 894
et 18/19 ms pour 8 094. Toutes les racines sont complètes ; aucune accélération
générale n'est démontrée par ces petits corpus.

La recette réelle reste ciblée sur une seule racine déjà choisie. Elle mesure
354 743 entrées en 18,737 s, soit 7 546 957 824 octets alloués et 6 681 793 058
octets logiques. Cette racine est complète ; 70 autres sont non demandées. Le
reçu reste `partial` pour le périmètre source. Les compteurs stderr paraissent
à 5, 10 et 15 secondes, stdout reste du JSON valide et les artefacts correspondent
aux tailles et empreintes du reçu. Le passage ne mesure pas l'espace récupérable
APFS et ne constitue pas un benchmark avant/après. Les chemins et rapports réels
restent locaux.

Validation locale : 375 tests Rust réussis, trois ignorés ; formatage et Clippy
sans avertissement, compilation release, six tests d'exclusion et deux tests du
harness de comparaison réussis. Après le dernier ajout de progression entre les
racines, les 13 tests disque, dix tests CLI et le test de cadence repassent.
La suite Python finale exécute 298 tests : 292 réussis et six ignorés. La première
exécution avait détecté une reconstruction du binaire pendant son oracle ; la
suite a été entièrement relancée avec le binaire final figé.
Les 22 fichiers générés sont cohérents. Dans un cache de routage isolé, Claude
et Codex sélectionnent le skill sur ses 14 exemples positifs et aucun de ses
six contre-exemples ; cela ne prouve pas son activation native.

Le contrôle navigateur initial a rencontré un timeout de clic sans JavaScript.
Une relance diagnostique conserve les assertions et passe les huit variantes
bureau/mobile, clair/sombre et avec/sans JavaScript. L'échec initial n'est pas
reproduit ; aucune correction CSS n'est déduite de ce résultat. La relecture
indépendante du correctif disque ne relève plus de finding après correction.
Livré par #22 ; la CI du commit fusionné a réussi, voir
l’[état publié](#état-publié-et-suite-au-09102026).

La collecte Git groupée (#21), la reprise de découverte (#19) et l'intégration
de `getattrlistbulk` restent hors de ce correctif. Les verdicts et la disponibilité
de `apply` ne changent pas.

## Validation du lot HTML pour livraison, 08/10/2026

La branche `codex/html-report-polish` part du `main` fusionné par #15
(`0c3e1a59e380175991fb2ab7d539d1e87e04f67e`). Elle rassemble les finitions
HTML, les contrôles navigateur et la documentation suivis dans #16.

Sur ce contenu, les validations obligatoires passent : formatage, Clippy,
369 tests Rust réussis (3 ignorés), 298 cas Python (292 réussis, 6 ignorés),
compilation release et cohérence des 22 fichiers de configuration générés.
Les contrôles Chromium passent : 12 rendus de thème, copie et secours manuel,
4 dispositions de chemins, 8 variantes d’aide, puis 8 variantes de repli pour
la revue et 8 pour l’inventaire. La vérification documentaire couvre 233 liens
locaux, 4 blocs shell et les empreintes des 4 captures.

Le rapport régénéré conserve les 60 lignes et 126 blocs de preuves du rapport
sauvegardé ; les cinq fichiers source sont inchangés. Aucun nouveau scan réel.
Safari, Firefox, les lecteurs d’écran et la copie native hors du presse-papiers
instrumenté ne sont pas vérifiés. Ce lot a ensuite été fusionné par #18,
avec CI réussie ; #16 et #17 sont closes. L’[état publié](#état-publié-et-suite-au-09102026)
conserve les liens. Les entrées ci-dessous décrivent les contrôles antérieurs.

## Fonds des grandes sections, 08/10/2026

Les cinq sections repliables partagent désormais le même fond, les bordures,
les arrondis et les marges internes. Les anciens styles sans carte de la
synthèse et des familles Git sont retirés. Le bilan disque garde sa bordure
supérieure orange. Le changement produit porte uniquement sur la feuille de
styles HTML ; le guide et le changelog sont alignés.

La compilation release, les 12 rendus du contrôle de thème et les 8 variantes
du contrôle de repli passent. Les captures repliées desktop/mobile, clair/sombre,
sont relues. Le HTML comparé à l’aperçu précédent ne diffère que par son CSS ;
les cinq fichiers source sont inchangés. Pas de nouvelle collecte, de suite des
collecteurs ni de vérification Safari/Firefox. Changements locaux, sans commit
ni push.

## Grandes sections repliables, 08/10/2026

Les cinq grandes sections HTML utilisent des contrôles natifs ouverts par
défaut : bilan disque, synthèse, familles Git, mesures disque et couverture.
Le clic, Entrée et Espace changent leur état sans JavaScript. Deux boutons
permettent de tout replier ou déplier avec JavaScript, en gardant l’état des
preuves imbriquées. Le menu et les liens internes rouvrent leur destination,
même lorsqu’on clique de nouveau sur l’ancre courante. Le message de lecture
seule reste visible. Les styles d’impression montrent les grandes sections
fermées sans modifier leur état à l’écran.

Le nouveau contrôle navigateur échoue sur l’ancien rapport puis passe sur
huit variantes de la revue et huit de l’inventaire : desktop/mobile,
clair/sombre, avec/sans JavaScript. Il vérifie clavier, liens, actions groupées,
conservation des preuves, filtres et impression. L’attente du contour de focus
natif est bornée à deux secondes ; l’exigence de contour n’est pas abaissée.
Les 12 rendus généraux du thème et les 8 variantes des aides du menu passent.
Les quatre captures repliées desktop/mobile, clair/sombre, sont relues ; les
deux actions groupées restent côte à côte sur mobile.
Les 14 tests Rust HTML, les 5 tests Python de publication, Clippy, formatage,
compilation release et cohérence des 22 fichiers générés passent.

Les données sauvegardées restent identiques : 60 lignes, 126 blocs de preuves
et cinq fichiers source inchangés. Aucun scan réel, aucune modification de
verdict. Les suites générales des collecteurs, Safari, Firefox et les lecteurs
d’écran ne sont pas vérifiés dans cette passe. Changements locaux, sans commit
ni push.

## Bilan disque en tête du rapport, 08/10/2026

Le HTML présente d’abord l’allocation dédupliquée observée, la couverture des
mesures et le nombre de candidats. Le gain récupérable reste **Not established** :
les blocs partagés APFS et les snapshots empêchent de le déduire de ces tailles.
Un repli liste les trois plus gros worktrees mesurés, avec chemins copiables et
mesures partielles signalées. Le Git commun est exclu de cette liste ; les
tailles des lignes ne sont pas additionnées. Les filtres gardent le périmètre
disque source. Sans mesure, le rapport indique **Not measured** ; un zéro
réellement mesuré reste affiché comme zéro.

Sur les observations sauvegardées du rapport à 60 worktrees, le bloc affiche
36,12 Go partiels, 8 racines complètes, 1 partielle et 52 non mesurées. Il reprend
0 candidat et 50 worktrees à vérifier. Les 60 lignes, les 126 blocs de preuves
et les cinq fichiers source sont inchangés. Aucune nouvelle collecte réelle.

Les deux nouveaux tests de comportement échouent avant l’ajout du bloc puis
passent. Les 14 tests Rust HTML, 5 tests Python de publication et 12 variantes
générales Chromium passent. Quatre contrôles du nouveau repli vérifient l’ordre
contre le JSON source, la copie exacte des chemins, les filtres et la largeur
mobile. Les captures clair/sombre desktop/mobile sont relues. Formatage,
Clippy, compilation release et cohérence des 22 fichiers générés passent.
Les suites générales Git et les autres navigateurs ne sont pas rejoués.
Changements locaux, sans commit ni push.

## Aides de navigation, 08/10/2026

Les quatre entrées du menu expliquent leur contenu au survol et au focus. Un
bouton **?** distinct sert au clic et au tactile. Les popovers natifs évitent
le rognage par le menu ; Échap et le clic extérieur les ferment. Les liens
gardent leurs ancres. Sans JavaScript, les boutons ouvrent une aide native ;
le survol et le placement près du lien sont propres au mode JavaScript.

Les huit variantes du nouveau contrôle navigateur passent : 1440/390 px,
clair/sombre, avec/sans JavaScript. Les quatre captures du mode JavaScript ont
été relues. Les 12 tests Rust HTML, les 5 tests Python de publication et les
12 variantes générales du thème passent. Formatage, Clippy, compilation release
et cohérence des 22 fichiers générés passent également. Les relations ARIA
sont vérifiées structurellement ; aucun lecteur d’écran n’a été exécuté.

L’aperçu utilise les mêmes observations : 60 lignes et 126 blocs de preuves
identiques, cinq fichiers source inchangés. Pas de nouvelle collecte réelle.
Les suites générales Git, Safari et Firefox ne sont pas rejoués. Changements
locaux, sans commit ni push.

## Largeur du rapport, 08/10/2026

Le conteneur principal n’est plus plafonné à 1200 px. Il prend la largeur
disponible à droite du menu, avec les marges internes prévues sur desktop et
mobile. Le contrôle du thème couvre désormais 2560, 1440 et 390 px et vérifie
les deux bords du conteneur. Il échoue sur l’ancien rendu plafonné.

Les douze rendus Chromium passent : trois largeurs, deux thèmes, avec et sans
JavaScript. Les captures large/mobile ont été relues dans les deux thèmes.
Les 12 tests Rust HTML passent et le binaire release est reconstruit. Le test
d’ouverture des preuves attend depuis Node : une attente animée dans la page
pouvait expirer sans JavaScript alors que la largeur finale était correcte.
Le seuil d’agrandissement reste identique.

L’aperçu est régénéré depuis les mêmes données à 60 worktrees. La comparaison
avec l’aperçu précédent confirme que seule la feuille de styles change ; les
cinq fichiers du rapport source sont inchangés. Pas de collecte réelle, de
nouveau test des collecteurs ni de validation Safari/Firefox pour cette passe.
Changements locaux, sans commit ni push.

## Disposition des chemins et boutons, 08/10/2026

Le défaut signalé plaçait le bouton de copie entre le chemin et le motif de
lacune. Les trois listes concernées (disque, couverture, manifestes locaux)
utilisent désormais une ligne chemin/bouton puis une ligne pour le motif.
Sur mobile, le bouton passe sous le chemin. Les détails disque occupent toute
la largeur ; leur bouton principal reste séparé du contrôle de dépliage.

Le nouveau test navigateur échoue sur le rapport d’origine puis passe sur
quatre rendus Chromium : 1440/390 px, clair/sombre. Les captures ont été relues.
Les 12 tests Rust HTML et les 5 tests Python de publication passent, ainsi que
formatage, Clippy, compilation release et cohérence des 22 fichiers générés.
Le contrôle de copie passe. Le test général du thème a d’abord échoué sur une
assertion immédiate de largeur ; un passage de diagnostic a mesuré 204 puis
431 px, au-dessus du seuil. Le test attend désormais cette largeur avec un
délai borné, sans abaisser le seuil. Les huit rendus du test final passent.

Un nouvel aperçu utilise les données sauvegardées du rapport à 60 worktrees :
les attributs des 60 lignes et les 126 blocs de preuves sont identiques. Les
cinq fichiers du rapport source sont inchangés. Aucune nouvelle collecte ni
mesure disque n’a été lancée sur les dépôts personnels. Les suites générales
Git, Safari et Firefox n’ont pas été rejoués pour ce correctif de présentation.
Changements locaux, sans commit ni push.

## Positionnement du README, 08/10/2026

Les deux README partent désormais du besoin donné par Florian : plus de place
sur le disque. L’accroche précède le bloc auteur ; le problème et les résultats
attendus précèdent les captures. Trois entrées orientent vers les parcours
existants : tailles disque, worktrees dispersés et vérifications avant nettoyage.

La structure s’inspire du
[README Search Console MCP](https://github.com/FlorianBruniaux/google-search-console-mcp#readme),
lu le 08/10/2026 : besoin utilisateur, parcours et résultat avant les détails
techniques. Les formulations et résultats appartiennent à Sweep ; aucune
promesse de place libérée ni de suppression automatique n’est ajoutée.
Les captures, commandes et contrats restent inchangés. Cette passe porte sur
la documentation, sans nouveau test produit ni scan réel.

## README et captures du rapport, 08/10/2026

Les README français et anglais présentent le HTML, les unités décimales et la
copie des chemins. Leurs schémas Mermaid nomment les sorties HTML, texte et JSON.
La roadmap distingue le jalon #8 clos, les finitions locales #16, le blocage CI
#17 et la vision #14. Les anciens chiffres de recette restent dans les sections
historiques ; le bilan courant cite le checkpoint intégré à 63 inscriptions.

Le visuel principal devient une capture native du générateur sur quatre worktrees
synthétiques : trois `keep`, un `cleanup_candidate`. Les captures clair/sombre
montrent le bilan, puis les lignes dans un repli. Les fichiers des fixtures sont
inchangés après la revue, les quatre verdicts HTML concordent avec le JSON et les
reçus sont vérifiés. Le manifeste de capture est dans
[les sources visuelles](assets/report-captures.json). Aucun dépôt personnel n’a
été scanné ; aucun appel de génération d’image n’a été effectué.

Les quatre captures ont été relues. Chromium ne signale ni erreur, requête
externe ni débordement horizontal dans le viewport de 1440 px. Le contrôle
documentaire vérifie 227 liens locaux, quatre blocs shell et les empreintes
des quatre images. Le rendu GitHub et Mermaid sur GitHub n’ont pas été rejoués ;
la mise à jour des schémas porte sur leurs libellés. Les suites générales ne
sont pas rejouées pour ces modifications documentaires. Changements locaux,
sans commit ni push.

## Copie des chemins dans le HTML, 08/10/2026

Les chemins du rapport ont un bouton **Copy path**. La copie utilise les octets
enregistrés, sans transformer les échappements d’affichage en chemin. Les
chemins UTF-8 conservent espaces, accents et guillemets. Un refus du presse-papiers
ouvre un champ sélectionné pour la copie manuelle ; les octets non UTF-8 et les
caractères de contrôle donnent un message explicite.

Les 12 tests Rust de rendu HTML et les 5 tests Python de publication passent,
ainsi que formatage, Clippy et compilation release. Le premier lancement Python
utilisait un interpréteur trop ancien pour `hashlib.file_digest` ; la reprise
avec le runtime Python fourni passe. Le test Chromium de copie couvre succès et
refus avec un presse-papiers instrumenté, repli manuel, clavier, mobile et absence
de JavaScript. Les huit rendus du thème passent ; les vues disque et copie
manuelle ont été relues sur desktop et mobile.

L’aperçu reprend le JSON du checkpoint précédent : ses 63 lignes et 133 blocs de
preuves sont conservés. Il n’y a ni nouvelle collecte ni modification des reçus
existants. Les suites générales Git n’ont pas été rejouées pour cet ajout de
présentation. Safari, Firefox et leur presse-papiers natif ne sont pas vérifiés.
Changements locaux, sans commit ni push.

## Tailles lisibles dans le HTML, 08/10/2026

Le rapport affiche les tailles en o, Ko, Mo, Go, To, Po ou Eo, en base 1000 et
avec deux décimales à partir du Ko. L’arrondi se fait en entiers, jusqu’à la
valeur maximale `u64`. Les octets exacts restent dans le JSON, les infobulles
et les totaux dépliables ; le tri utilise toujours les octets bruts.

Les 11 tests de rendu HTML passent, dont les seuils d’unité, les arrondis, zéro,
les mesures absentes et les tailles maximales. Les 5 tests Python de publication
HTML passent, ainsi que formatage, Clippy et compilation release. Les huit
rendus navigateur du thème passent avec les unités ; les totaux et leurs détails
exacts ont aussi été contrôlés sur desktop et mobile. Les 63 lignes et les
133 blocs de preuves de l’aperçu sont conservés. Les suites générales Git du
checkpoint précédent n’ont pas été rejouées pour ce changement d’affichage.

## Thème BoldGuy, 08/10/2026

Le générateur HTML utilise les fonds crème/noir, l’accent orange, les polices
système et les métriques monospace de BoldGuy. Le choix clair/sombre est restauré
avant le contenu quand le stockage local est autorisé. Le stockage refusé et
l’absence de JavaScript gardent un affichage utilisable. Sur grand écran, une
preuve ouverte reçoit davantage de largeur dans le tableau.

Un aperçu a été régénéré depuis le `review.json` du contrôle réel précédent :
**63 worktrees, 12 à conserver, 51 à vérifier, aucun candidat**. À ce checkpoint,
avant la conversion des unités ci-dessus, les contenus HTML hors styles et
scripts étaient identiques au rapport d’origine. Le JSON et le HTML
d’origine correspondent toujours à leur reçu. Cette validation ne constitue pas
une nouvelle collecte ni une nouvelle mesure disque.

Les huit rendus Chromium combinent 1440/390 px, clair/sombre et JavaScript
activé/désactivé. Les captures desktop et mobile ont été relues. Les contrôles couvrent recherche,
filtre, tri, clavier, cibles de 44 px, contrastes de texte échantillonnés, mouvement
réduit, préférence après rechargement, stockage refusé et fond d’impression.
Safari et Firefox ne sont pas vérifiés ; ces contrôles ne constituent pas un
audit d’accessibilité exhaustif. Le script facultatif `tests/html-theme.cjs` et
son utilisation sont documentés dans le [guide HTML](html-report-plan.md#vérifier-le-thème).

La suite Rust complète passe : **365 réussites, 3 tests ignorés**. Après le dernier
ajustement CSS, les **10 tests de rendu HTML** passent de nouveau. Les **5 tests
Python de publication HTML** passent sur le binaire reconstruit. Formatage,
Clippy, compilation release et cohérence des 22 fichiers générés passent.
La suite Python couvre ses 298 cas sur le passage interrompu puis la reprise
des cas restants : **292 réussites, 6 ignorés, aucun échec**. Le contrôle
d’ordre sur 46 et 100 worktrees passe lors de cette reprise.
Les changements de thème restent locaux à ce checkpoint, sans commit ni push.

## Reprise du 07/10/2026

Le checkout de travail précédent était absent. L’instantané Git
`14f00ea454795fd6df1f86dec72ec6573d3e546e`, créé par Codex sous le libellé
`startup-cleanup`, conserve les ajouts disque et métriques. La branche
`codex/html-reports` reprend cet instantané dans un checkout isolé. Le dossier
principal et ses modifications locales sont préservés.

Les 24 tests CLI, disque et jugement rejoués sur cet état passent. Ce passage
ne remplace pas la suite complète du 06/10 et ne résout pas l’écart du test à
100 worktrees. Cet état de reprise précède le lot d’audit lisible décrit plus
bas : HTML implémenté et cause de l’écart de collecte reproduite.

Le contrôle GitHub du 07/10 confirme les PR 1 à 7 fusionnées, aucune PR ouverte
et `main` à `31b58339c6d4a84750bbbad2c0f89ebfa4c8624f`. Les ajouts disque et
les dernières métriques restent dans l’instantané local, sans PR de publication.

## Affichage terminal, 07/10/2026

La CLI met à jour une seule ligne de progression dans un terminal compatible, en respectant sa largeur.
Une redirection conserve les étapes principales. `--summary` affiche les
verdicts, la couverture et les tailles ; `--verbose` conserve les événements
par dossier et les métriques détaillées. Les couleurs et icônes sont automatiques,
avec `--color auto|always|never`, `NO_COLOR` et `TERM=dumb`. Les mesures partielles
restent signalées et les contrôles manquants du bilan court portent sur les
lignes sélectionnées. Le rapport complet et les artefacts JSON restent sans
décoration. La collecte et les règles de jugement ne sont pas modifiées.

Les changements portent sur `src/main.rs`, `src/terminal.rs` et le rendu dans
`src/presentation/`. Les README anglais et français, la référence CLI, le tutoriel,
le guide disque et le changelog documentent les options. Les tests utilisent des
dépôts jetables ; aucun scan de la machine ou des dépôts personnels n’a été relancé.

Les six scénarios initiaux d’affichage échouent avant implémentation puis passent :
progression compacte, récapitulatif court, mode détaillé, codes de couleur absents des
artefacts, détection indépendante des deux terminaux et désactivation de la
décoration. Les pseudo-terminaux sont réels. Deux tests du rendu couvrent les
mesures partielles, les filtres et les conditions manquantes. Sur la même fixture
à deux worktrees, la progression redirigée passe de 18 à 6 lignes ; le nouveau
bilan mesure 24 lignes sans dossier de publication.

Le passage Rust complet termine avec **342 tests réussis, 3 ignorés**. La suite
Python exécute **284 cas, 6 ignorés, un échec** : le test de publication attendait
le titre supprimé `CHECKS STILL NEEDED` alors que les deux verdicts étaient résolus.
Ses assertions suivent désormais le bilan compact, avec les mêmes verdicts et
la même couverture attendus. Les scénarios d’ordre sur 46 et 100 worktrees passent
lors de ce rejeu ; ce passage seul ne démontre pas la disparition de l’écart
intermittent observé le 06/10.

Un test supplémentaire reproduit ensuite le repli de la progression à
32 colonnes. Le correctif lit la largeur courante via la fonction `termios` de
`rustix`, déjà présent dans le projet. Après cet ajustement, **31 tests Rust
ciblés et 9 tests Python passent** : les huit tests d’affichage et le test de
publication corrigé. Les essais de largeur couvrent 2, 4, 32 et 80 colonnes,
ainsi qu’une largeur non renseignée. La ligne est effacée avant un diagnostic
d’erreur. Les suites globales ne sont pas intégralement rejouées après cet
ajustement d’affichage.

Le formatage, Clippy, la compilation release, les 22 projections d’instructions
et les 132 liens locaux contrôlés dans les documents modifiés passent. Les
changements restent locaux, sans commit ni publication GitHub pour ce lot.

## État courant au 06/10/2026

U1-U4 et T01-T09 sont livrés en lecture seule. Le triage a rejoint `main` par
la [PR #1](https://github.com/FlorianBruniaux/wt-sweep/pull/1), commit `ddda77d`.
`review` regroupe découverte et revue ; le rapport distingue `keep`,
`cleanup_candidate` et `verify`, avec cibles, faits locaux, histoire et conditions
manquantes. Le [checkpoint T01-T09](finish-ledger.md#triage-t01-t09) contient les
vérifications finales et l'essai réel : 48 worktrees, 3 `keep`, 45 `verify`, aucun
candidat. Ce résultat historique précède les profils de collecte décrits ci-dessous.

La [livraison GitHub](finish-ledger.md#livraison-github-du-06102026) sépare la CI
réussie avant fusion de la vérification du commit fusionné. La
[feuille de route](roadmap.md) garde le diagnostic de performance prioritaire.
E1 reste non démontrée et `apply` indisponible.

## Espace disque, 06/10/2026

Le checkout local ajoute `scan --disk-usage`, `review --disk-usage` et
`--disk-budget standard|thorough`. Le rapport final et le récapitulatif console
rendent les mesures capturées directement depuis la CLI. `plan` les conserve
avec leur intervalle d’origine ; les filtres gardent les totaux de la source.
Une absence de mesure est affichée explicitement.
Le collecteur mesure les métadonnées par descripteur, sans lire le contenu des
fichiers. Les tailles allouées et logiques sont publiées par dossier de travail
et administration Git, avec total dédupliqué par périphérique/inode. Les autres
racines enregistrées imbriquées sont comptées séparément. Le tri du rapport par
taille conserve branche, chemin et dépôt ; les lacunes restent explicites.
Le [guide disque](disk-usage.md) précise les plafonds et la limite APFS.

Les sept scénarios initiaux du collecteur passent : fichiers ignorés et creux, liens symboliques,
liens physiques locaux et partagés, racines imbriquées, budgets, racine absente ou
remplacée, noms avec caractères de contrôle et dépôt nu. Les neuf tests CLI
passent, y compris le refus d’un profil invalide et l’acceptation de `thorough`.
Une première fixture utilisait un nom non UTF-8 refusé par APFS ; elle utilise
maintenant tabulation et saut de ligne. Ce contrôle ne prouve pas la gestion
native de noms non UTF-8 sur un autre système de fichiers.

Avant l’intégration au rapport de revue, la suite Rust exécutée avec un seul test à la fois passe : 336 tests,
plus trois ignorés. Une exécution concurrente avait dépassé le délai du test de
transport de 32 Mio ; son rejeu isolé passe en 5,66 s sans modifier le seuil.
La suite Python complète a exécuté 277 cas, avec six ignorés et deux échecs de
sous-scénarios dans le test d’ordre sur 46 et 100 worktrees. L’écart porte sur
`cleanup_candidate` contre `verify` entre deux collectes. Ce passage n’est pas
vert. La charge de la machine et des collectes concurrentes était élevée ; cela
ne suffit pas à attribuer la cause. Le rejeu isolé termine en 443,879 s : le
sous-scénario à 46 worktrees passe, celui à 100 conserve l’écart de verdict.
Ce constat reste ouvert ; la validation Python globale n’est pas acquise.
Les tests et les plafonds n’ont pas été modifiés pour contourner cet échec.
Formatage, Clippy et build release passent à ce checkpoint. Le SHA-256 de ce binaire release
est `3bd01ef53f49d386edbbba999f7c5ceab8d8654c1dedf935a0896e32705d61db`.

Les README, aide CLI, contrats, changelog, index et roadmap sont alignés. Les
22 projections d’instructions restent synchronisées. Les 179 liens locaux et
sept blocs shell contrôlés sont valides. La publication GitHub reste à faire.
Les mesures personnelles restent hors du dépôt.

### Raccordement au rapport final

`review` accepte la mesure dans la même commande que l’analyse. Les tests ajoutés
vérifient la publication native des tailles dans la console, le texte et le JSON,
leur conservation par `plan` après modification des fichiers, le même rendu pour
les mêmes observations et l’absence d’effet sur un verdict positif de candidat.
Une vue filtrée conserve toutes les mesures et n’affiche le bilan qu’une fois.
Les métriques précisent que la mesure disque a sa propre durée.

La suite Rust complète passe avec 340 tests et 3 ignorés. Les deux derniers
libellés de temps ont ensuite été revérifiés par 26 tests ciblés du rendu, du
jugement et de la mesure. Les 32 scénarios Python de revue et de publication
passent également. Formatage, Clippy et build release
passent. Un essai release sur deux dossiers de travail et une administration Git
jetables retrouve les octets alloués de l’oracle `stat` et publie les mêmes données
dans `inventory.json` et `review.json`. Le binaire de ce raccordement a pour SHA-256
`dec98c6dea3accf9fe74b009eeb9b25fc5ff731b431a5cfb09659985182d2826`.
Ces contrôles n’effacent pas l’échec d’ordre à 100 worktrees décrit plus haut.

## Indicateurs de performance, 06/10/2026

Le checkout local `codex/scan-performance` ajoute durée monotone, tentatives de
lecture de dossiers, entrées listées et workers dans les métriques de découverte.
La console affiche le débit et la couverture séparément. Les revues ajoutent le
débit local, les appels Git mesurés et la part de la phase la plus longue.
Le [benchmark Rust](collection-performance.md#benchmark-rust-reproductible)
compare les workers sur une fixture constante avec vérification de conservation.

Les tests ont d’abord échoué sur l’absence de métriques et leur rejet par le
lecteur, puis passé après raccordement. La suite complète compte 327 tests Rust
réussis et trois ignorés, dont le benchmark exécuté séparément avec succès.
La suite Python 3.14 compte 277 cas : 271 réussis et six ignorés. Une première
invocation avec Python 3.9 a échoué sur `hashlib.file_digest` et a été interrompue ;
elle ne fait pas partie du résultat de validation. Formatage, Clippy et contrôle
des 22 projections passent. Les 373 liens documentaires locaux existent et les
six blocs shell des README et guides modifiés sont syntaxiquement valides.

La mesure isolée donne une médiane de 1,043 s à un worker et 1,157 s à quatre,
pour 2 218 entrées avec couverture complète dans les deux cas. Les plages se
chevauchent ; aucun gain du parallélisme n’est établi. Le
[registre des mesures](collection-performance.md#mesure-locale-du-06102026)
conserve le corpus, les empreintes et les limites. Aucun scan global réel ni
publication distante n’a été exécuté pour ce lot. Les 60 secondes de découverte
et l’absence de reprise restent des limites distinctes.

## Profils et grands scénarios A1-A6, 06/10/2026

Le lot ajoute les profils finis `standard|thorough`, les plafonds appliqués et
les métriques par phase. Les vérifications approfondies groupent les blobs ;
l’histoire privée partage son parcours et donne 32 expansions prioritaires aux
ancres déjà utiles. Le transport Git attend les descripteurs disponibles au
lieu de temporiser entre chaque petite rafale de sortie. C1-C5 restent inchangées.

Le scénario synthétique de 4 200 blobs distincts, 600 commits et 32 racines privées
obtient un candidat complet. Une famille de 50 worktrees donne 48 candidats et
deux rétentions justifiées, avec conservation indépendante de la source. Sur
une sonde constante de 64 Mio répartis entre 16 enfants Git, la collecte passe
de 46 540 ms à 500 ms. Ce résultat de transport ne mesure pas la revue complète.

Le nouveau passage réel ciblé établit une ascendance, couvre les deux racines
privées et classe le worktree `keep` : une modification non commitée et au moins
33 741 fichiers ignorés sont observés. Les 52 213 chemins examinés ne constituent
pas un inventaire complet ; le plafond local d’octets laisse C2, C3 et C5
incomplètes. La phase privée dure 5 063 ms sur ce passage, contre 240 473 ms sur
le diagnostic précédent. Le dépôt évolue entre les deux observations.

Les README, contrats, tutoriel, aide CLI, roadmap et skill expliquent le passage
ciblé et les limites restantes. Les projections Claude/Codex sont synchronisées ;
le routage BM25 est vérifié sur les deux clients. Le chargement dans une nouvelle
session native reste non vérifié. Le [registre de collecte](collection-performance.md)
conserve les observations distinctes et leurs identités de binaire.

Le passage familial thorough conserve les 50 lignes : 49 `keep`, un `verify`,
aucun candidat. L’histoire des commits est collectée pour les 50 ; dix résumés
locaux sont complets et quarante restent partiels. Les phases locale et privée
atteignent leurs plafonds. Les 49 rétentions ont des motifs observés, dont des
fichiers ignorés ; elles ne signifient pas 49 analyses exhaustives. Le dernier
`verify` garde notamment un index intent-to-add non pris en charge et une
histoire privée incomplète. Les artefacts du run sont vérifiés contre leur reçu.

La revue indépendante est statique : elle couvre C1-C5, les budgets, les témoins
privés, l’attente des flux Git et la conservation des causes d’arrêt. Aucun
constat de cette revue ne reste ouvert. Formatage et Clippy passent. La suite
Rust finale compte 324 réussites et deux cas ignorés ; la suite Python compte
271 réussites et six cas ignorés. Les 6 expériences d’exclusion passent,
sans démontrer E1. Le binaire release est reconstruit ; le
[registre de validation](finish-ledger.md#complétion-des-gros-dépôts-a1-a6)
conserve son empreinte et les limites des contrôles.

## Collecte des gros dépôts, 06/10/2026

Les lectures locales d’arbres et de blobs sont groupées et dimensionnées avant
exécution. Le graphe d’une cible vérifiée est partagé entre worktrees de la même
famille physique. Le temps local restant est réparti entre les worktrees ;
`--worktree` permet un passage concentré sur un chemin, tout en conservant
l’inventaire et les omissions explicites. Les README, le tutoriel, les contrats,
l’aide CLI et les instructions Claude/Codex générées décrivent ce comportement.

Les fixtures distinctes passent avec 23 processus Git pour 1 024 blobs et au plus
40 lectures d’objets pour 1 024 dossiers. Un graphe partagé de 12 commits demande
12 expansions. Les délais de phase d’histoire et d’histoire privée n’effacent
plus les preuves indépendantes comme le ferait un vrai timeout du backend.
Les revalidations manquantes restent inconnues.

Le dernier passage complet compte 49 worktrees : 4 `keep`, 45 `verify`, aucun
candidat. Il conserve trois ascendances vérifiées, sans fausse quarantaine ;
les 49 résumés locaux restent partiels. Le passage ciblé vérifie une ascendance
et observe au moins 4 969 fichiers ignorés, donc classe ce worktree `keep`. Les plafonds d’objets et d’histoire privée
laissent encore des contrôles incomplets. Le [registre de collecte](collection-performance.md)
détaille les fixtures, la revue indépendante et les limites des mesures réelles.

Validation du lot : 303 tests Rust réussis, 2 ignorés ; 260 tests Python réussis
sur 266, 6 ignorés ; six expériences de conservation réussies. Formatage, Clippy,
builds et synchronisation des 22 fichiers générés passent. Le contrôle
documentaire couvre 31 fichiers, 327 liens locaux et 38 blocs shell. La revue
indépendante est statique ; les suites intégrées ont été exécutées par
l’intégrateur. La validation distante reste distincte de ces résultats locaux.

## Bilan console, 06/10/2026

`--summary` affiche les verdicts, la couverture du périmètre source, le nombre de
contrôles locaux complets, partiels ou absents et les conditions encore manquantes.
Les budgets d’historique, budgets locaux et timeouts Git sont regroupés. Les
messages de progression sont lisibles ; le JSON et les règles de décision restent
inchangés. Ce changement d’affichage ne résout pas les limites de performance
observées sur le gros dépôt.

Vérifications locales : 280 tests Rust réussis, 2 ignorés ; formatage, Clippy,
synchronisation des 12 fichiers d’instructions et 6 tests RETEX réussis.
La suite Python complète a exécuté 253 cas : 246 réussis, 6 ignorés et une ancienne
assertion `processed=1` à mettre à jour. Après cette correction et l’accord des
messages au singulier, les 9 tests du module concerné passent. La suite Python
complète n’a pas été rejouée après ces derniers changements.
Le rendu du rapport existant de 50 worktrees donne 3 à conserver, 0 candidat et
47 à vérifier, avec 0 contrôle local complet, 4 partiels et 46 non collectés.
Aucune nouvelle analyse du dépôt réel n’a été lancée pour cette vérification.

## Skill wt-sweep, 06/10/2026

Le [skill utilisateur](assistant-skill.md) est intégré au harnais du dépôt.
Il vérifie l’aide CLI, choisit le périmètre et les cibles explicites, lit les
rapports par pages et propose des actions de conservation ou de vérification.
Les README anglais/français, le tutoriel et la feuille de route le référencent.
`apply` reste indisponible ; le skill ne produit pas de suppression de remplacement.

La source `harness/skills/wt-sweep` est projetée à l’identique dans les dossiers
natifs Claude et Codex. Les 20 fichiers générés sont synchronisés. Les tests du
générateur couvrent aussi les ressources absentes, liens sortants, préimages
modifiées et skills voisins à préserver.

Essai comportemental sur une famille Git jetable : le principal est `keep` et
le lié propre `cleanup_candidate`. L’ajout d’un fichier non suivi fait passer le
lié à `keep`. La lecture paginée conserve les deux identités ; une vue filtrée
à un candidat garde le total source de deux. Les longueurs et SHA-256 des artefacts
correspondent au reçu. Les manifestes de fichiers et métadonnées avant/après
restent identiques, hors atime exclu par l’oracle. Aucun dépôt réel n’a été scanné.

Routage BM25 local vérifié pour les deux clients : 12 scénarios positifs et
6 négatifs, skill éligible, seuil de qualité global satisfait. F1 de calibration
0,923 ; F1 d’évaluation croisée 0,649 ; F1 global 0,739. Ces scores ne constituent
pas une absence de faux positifs. Les 12 sondes de routage exécutées par client
passent : quatre demandes positives nouvelles en français/anglais et huit
négatives, dont les six du corpus. Les caches réels ont été reconstruits pour ce
checkout, après essai dans un cache temporaire. Aucun hook ou réglage global
n’a changé.

Validation locale : formatage, Clippy et compilation réussis ; 280 tests Rust
réussis, 2 ignorés. La suite Python a exécuté 260 cas : 254 réussis et 6 ignorés.
Les deux tests du parcours du skill et les 14 tests du générateur ont aussi été
exécutés séparément avec succès après les derniers ajustements. Les 6 tests
RETEX passent. Le contrôle documentaire couvre 28 documents, 306 liens locaux
et 33 blocs shell. Ces résultats ne prouvent pas une exécution de la CI distante.

La présence des projections et le routage sont observés. Le chargement et
l’invocation du nouveau skill dans une session native fraîche restent `UNKNOWN`.
L’essai CLI ci-dessus est un test des recettes, pas une évaluation d’un modèle
chargé automatiquement par le client.

## Choix des modèles du skill, 06/10/2026

La [politique de modèles](../harness/skills/wt-sweep/references/model-policy.md)
précise quatre profils, de l’extraction factuelle à la revue indépendante. Elle
vérifie les capacités du client avant de demander un modèle ou un niveau de
raisonnement, respecte les choix de l’utilisateur et distingue une recommandation,
une invocation acceptée et un réglage effectif confirmé. Les réglages persistants
et les modèles des rôles de développement restent inchangés.

Deux sous-agents au maximum peuvent lire des lots indépendants de rapports déjà
collectés, sous réserve des places disponibles. Le coordinateur garde la collecte,
les stashes partagés et la synthèse. Une preuve absente n’est pas un motif pour
augmenter le raisonnement. Les recommandations de profils n’ont pas fait l’objet
d’un benchmark de qualité ou de coût.

Une revue séparée a été exécutée avec `gpt-6.1-sol` et `high` demandés au lancement.
Le sous-agent a relu les règles et six scénarios : rapport simple, lots parallèles,
contrôle indisponible, budget Git épuisé, retour partiel ou verdict altéré, choix
explicite de l’utilisateur avec une place occupée. Aucun défaut actionnable n’a
été signalé. L’acceptation des arguments est observée ; cette revue ne mesure ni
le modèle effectif du fournisseur, ni le chargement natif du skill, ni sa qualité
comparée sur des rapports réels.

Après cet ajustement documentaire, les 14 tests du générateur et les 2 tests du
parcours CLI passent. Les 22 fichiers générés sont synchronisés. Les deux caches
BM25 ont été reconstruits : skill éligible, seuil global satisfait, 12 sondes sur
12 réussies par client. Le contrôle documentaire couvre 29 documents, 312 liens
locaux et 33 blocs shell. Les suites produit complètes
ne sont pas rejouées pour ce changement d’instructions ; leurs résultats au
commit précédent sont consignés ci-dessus.

## Historique U1-U4 et RETEX, 04-05/10/2026

Les sections suivantes conservent les résultats, l'état Git et les limites de
leurs dates d'observation. Leurs chiffres ne décrivent pas le triage courant.

Le 04/10/2026, le MVP macOS U1 à U4 est implémenté et vérifié sur des familles
Git jetables : inventaire, découverte, rapport local et import forge. La table
native des montages et la politique de refus de matérialisation sont raccordées.
La finition des 04 et 05/10 ajoute rapport descriptif, manifeste local explicite et dossiers
d'exécution ; ses preuves sont regroupées dans [finish-ledger.md](finish-ledger.md).
Le parcours global est testé avec volumes injectés ; aucun scan réel de la
machine n'a été exécuté. E1 reste `NON_DEMONTREE` et `apply` refuse l'exécution.

Autorité d'acceptation : [plan consolidé](plan.md). Répartition :
[implementation.md](implementation.md). Les observations locales ne valident
ni une CI distante, ni un scan de la machine, ni un retrait de dépôt.

## État initial et autorisation

Le socle recontrôlé avait 33 fichiers non suivis, une branche `main` sans HEAD,
5 tests Rust et 7 tests Python réussis. Formatage, Clippy et synchronisation des
12 projections passaient. Le checkout partagé a été conservé, avec un propriétaire
par fichier. À cette étape, aucun commit, push, fetch, prune, gc, nettoyage réel ou scan global
réel n'avait été exécuté. Les seules suppressions concernent les fixtures possédées
sous `/private/tmp`. Les sources historiques sont restées en lecture seule.

## Modèles et ownership observés

| Agent | Modèle et raisonnement | Fichiers et responsabilité |
|---|---|---|
| Intégrateur | Paramètres hérités de la session | Cargo, lib/model/main/output/review, reprise de discover et de ses tests pour la concurrence, tests CLI/modèle/sortie, documentation, CI et instructions canoniques |
| A, sweep-implementer | GPT-6.1 Sol, `high` | Collecteur Git et découverte initiale, `src/mounts.rs`, tests scan/mounts ; revue indépendante d'U4 et de residency |
| B, sweep-test-engineer | GPT-6.1 Sol, `high` | Oracles inventory/review/MVP/historical et helpers, `tests/review.rs` ; aucun code produit |
| C, sweep-implementer | GPT-6 Astra, `high` | Expérience E1, forge et reçus, `src/residency.rs` et ses tests ; revue indépendante d'U1/U3 et des montages |

L'API de collaboration expose et invoque ces rôles dans cette session. L'origine
exacte des définitions chargées et leur découverte automatique dans une nouvelle
session restent `UNKNOWN`. Aucun hook n'a été installé.

## U1 : inventaire exact d'une famille

`scan --repo` retourne principal, lié, détaché et inscription dont le dossier est
absent, avec l'ensemble exact attendu et des HEAD distincts. JSON préserve les
octets des chemins et les OID complets ; le tableau échappe les caractères de
contrôle. Le premier test réel échouait sur le refus du socle.

La revue a reproduit capture partageant un offset, écriture via TMPDIR hostile,
omission d'une administration cassée et substitution du parent de sortie.
Le collecteur contrôle maintenant les administrations, capture par sockets et
borne les appels Git. La sortie crée exclusivement un fichier, via descripteur
de répertoire et contrôle d'identité, hors du périmètre et des administrations.

L'oracle indépendant compare octets, modes, liens, mtime et ctime avant/après ;
l'atime est explicitement exclu. Les témoins fsmonitor/filtres/environnement
restent inactifs. EPERM/EACCES/EIO injectés ne deviennent pas une absence.
Un HEAD nul reste inconnu. La barrière avant U2 a passé 20 tests Rust, 21 tests
Python avec un cas sauté, E1, formatage, Clippy et synchronisation.

Le cas sauté demande un vrai nom non UTF-8 : ce système de fichiers refuse sa
création avec EILSEQ. Les octets non UTF-8 d'une inscription Git et du format
réversible sont testés. TCC réel, ACL et restauration des attributs restent inconnus.

La clôture du MVP a reproduit une mauvaise identification du principal avec
`git init --separate-git-dir`. Le collecteur vérifie désormais le toplevel,
l'administration commune et le pointeur `.git` avant de retenir le principal.
Un chemin d'administration sans backlink suffisant reste explicitement inconnu.
Les positifs principal séparé, linked et bare passent, y compris le rapport local.

## U2 : découverte et couverture

`scan --root` répétable et `scan --all` utilisent un parcours borné.
Les 14 tests de découverte couvrent `.git` fichier/dossier, bare à nom arbitraire,
administrations séparées, cachés, ignorés, dépôts imbriqués et sous-modules.
Ils comparent un ensemble exact non vide et un manifeste indépendant.

Alias, cycles, marqueurs cassés, accès refusés, limites et annulation sont
exposés. Les familles sont dédoublonnées par identité physique. Une observation
ultérieure contradictoire conserve ses faits et sa lacune. Les inscriptions hors
racines ont une origine explicite ; les liens sortants ne sont pas parcourus.
Volumes indisponibles et changements de montages sont éprouvés par injection.
Les contrôles dataless refusent les candidats concernés avant leur lecture Git.

Le fournisseur macOS lit maintenant tous les enregistrements `getfsstat` sans
filtrage silencieux, exige deux tables égales non saturées et conserve erreurs,
changements, source et type. Les 11 tests de montages couvrent fautes injectées
et lecture native de la seule table, sans parcourir les volumes. Un volume
disparu invalide la disponibilité de ses worktrees ; une table finale illisible
empêche de transformer cette incertitude en absence.

`--workers 1..8` règle les lectures parallèles de métadonnées, avec 1 par défaut.
Un témoin vérifie un chevauchement réel de quatre lectures et sa borne. Les
oracles comparent les mêmes faits à 1 et 4 workers, avec plusieurs budgets de
parcours ; l'agrégation reste déterministe. Les appels Git restent séquentiels.

La politique native de matérialisation dataless est désactivée et relue avant
les I/O, pour le processus et chaque thread. Son héritage par exec est testé,
afin de couvrir les chemins indirects de Git. Les includes, alternates et packs
résidents ont des cas positifs ; les FIFO déclenchent un refus ou un délai Git
borné, sans preuve d'intégration. Les [frontières natives](native-boundaries.md)
documentent l'ABI et les exceptions unsafe privées. Les deux frontières natives
initiales ont été relues mutuellement ; la sélection du cwd Git par descripteur
ajoutée en finition possède sa revue distincte.

Un véritable placeholder FileProvider, TCC et les montages non visibles restent
non éprouvés. Les observations ne sont pas un instantané atomique ; les appels
bloqués dans le noyau ne garantissent pas une interruption immédiate. `--all`
n'a jamais été lancé sur la machine de développement.

## U3 : relations locales et jugement pur

`plan --inventory` enrichit les faits, puis appelle un jugement sans I/O.
Les refs complètes `--target` désignent les cibles d'intégration ; les autres
restent intermédiaires. Aucune branche par défaut n'est devinée. Les relations
permises sont identité de commit, ascendance brute et identité d'arbre,
avec source, cible et OID explicites.

Les positifs produisent des observations non vides, y compris SHA-256.
Les cas S1 à S9, patch-id égal sur blocs répétés, grafts, replace, gitlinks masqués,
objets absents ou corrompus, inventaire périmé et commit tardif ne produisent
pas de fausse preuve. Les octets lus sont vérifiés contre leur OID, avec détection
de collisions SHA-1. Un marqueur shallow n'efface pas une ascendance brute
présente ; un parent manquant reste inconnu.

Les budgets bornent objets, octets, cibles et durée. Le schéma `wt-sweep.review/1`
refuse action/suggestion, reste déterministe à faits fixes et porte un digest
canonique. Un worktree déplacé depuis l'inventaire reste protégé contre une
sortie intérieure. Ce cas et un faux objet intermédiaire ont échoué avant correction.
Pour un périmètre global partiel, les ancêtres de sortie sont également inspectés
pour leurs marqueurs Git : une famille non encore découverte n'autorise pas
l'écriture dans son dossier. Le cas positif hors dépôt reste accepté.

Une MR ouverte précisément associée prime sur l'intégration, même dans un export
partiel. La tête source d'une MR fermée ou fusionnée n'est pas une cible.
Sans forge, son état reste inconnu ; l'âge seul ne crée pas d'activité.
`scan --local` collecte les fichiers et certains éléments d'administration avec
des limites explicites. Le rapport reprend ce manifeste en précisant que les
octets locaux ne sont pas revalidés ; ACL, attributs et reflogs partagés manquent.

## U4 : exports et reçus hors ligne

`forge commandes` affiche des modèles sans les exécuter. `forge ingerer` lit
exports GitLab et GitHub REST/GraphQL/CLI, pages concaténées ou groupées, et reçus
liés aux octets persistés. Un exécuteur de fixture produit réellement raw et reçu.

Les tests couvrent statut d'échec, saturation, troncature, page manquante,
curseur répété, versions contradictoires, projet homonyme/fork, remplacement
du raw et falsification de l'artefact normalisé. Les faits positifs partiels
restent disponibles. La revue indépendante a reproduit deux défauts : choix
silencieux entre champs contradictoires et omission d'une edge GraphQL sans node.
Ces cas ont échoué avant correction. Les contradictions invalident l'association
précise ; les edges incomplètes restent comptées et signalées. Un cas positif
d'alternatives concordantes empêche un refus systématique.

Pagination complète, cohérence temporelle et absence prouvée restent distinctes.
Les adaptateurs ne certifient aucune absence. Un reçu cohérent lie des octets
à des métadonnées déclarées ; il n'authentifie pas l'exécuteur. Le rapport relit
les preuves originales avant association. L'état actuel du serveur reste inconnu.

Un import historique autorisé, en lecture seule, a conservé 430 demandes issues
de cinq pages de tailles 100/100/100/100/30. Son test est ignoré par défaut et exige
un chemin explicite ; seules les fixtures synthétiques entrent dans le dépôt.

Quatre replays Python supplémentaires vérifient cet ensemble et les cas !91,
!92 de stats-api, puis la MR !383 associée au worktree `lead-app-mr43`. Le suffixe
du worktree n'est pas l'IID de sa MR. Les 104 entrées des deux pages stats et les
430 entrées lead-app sont conservées. Les trois SHA de fusion manquants restent
`null`, sans inventer de preuve depuis l'état de la MR ou sa tête source.
Le manifeste indépendant confirme la conservation des sources, hors atime.
Sans reçu d'exécuteur, la provenance reste partielle ; la disponibilité actuelle
des objets et l'intégration historique restent `UNKNOWN`. Aucun dépôt réel n'a
été consulté pour compléter ces absences. Ces replays exigent un chemin autorisé
via `WT_SWEEP_HISTORICAL_FORGE_DIR` et sont sautés par défaut.

## E1 et suite

La reprise des [retours msds et Aristote](retex-sessions.md) ajoute cinq tests
aux témoins E1 : ignorés supprimés sans force, non-suivis refusés, empreinte de
statut insensible au contenu, index distinct perdu par un patch depuis HEAD et
observation native d'un écrivain au cwd extérieur. Les six tests passent ;
aucune fonctionnalité de retrait n'est ajoutée. Les huit pistes sont évaluées
et trois lots de lecture seule sont raccordés pendant la finition.

La [décision E1](decisions/exclusion-et-conservation.md) documente les témoins
réels : descripteur extérieur après renommage/verrou coopératif, écriture à
taille/mtime identiques et commit détaché tardif. Le test valide des contre-exemples,
sans prouver l'exclusion. ACL/attributs/types spéciaux, restauration indépendante
et durabilité restent `UNKNOWN`. U5 à U7 ne sont pas implémentées.
La [sonde de capacités natives](decisions/apply-readiness.md) constate EPERM
sur le lease d'écriture, ENOTSUP sur revoke d'un fichier régulier, et l'écriture
toujours possible via le descripteur préexistant. Aucun privilège ou montage
n'a été modifié. Il faut éprouver une frontière d'autorité distincte avant U5.

Le mandat du MVP U1 à U4 est terminé sur les fixtures et frontières décrites.
U5 exige séparément E1 concluante et la comparaison des alternatives aux mêmes
fixtures. Aucun succès des tests actuels ne lève cette condition.

## Vérification intégrée avant les lots du RETEX

Vérification finale du 04/10/2026, après raccordement des frontières natives,
concurrence, corrections des administrations séparées et des sorties globales :

| Contrôle exécuté | Résultat observé |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --all-targets --locked --offline -- -D warnings` | PASS |
| `cargo test --locked --offline` | 80 passés ; 2 ignorés : historique explicite et entrée de sous-processus appelée par son test parent |
| Test historique, chemin explicite et `--ignored` | 1 passé, 430 demandes conservées |
| Replays Python, chemin historique explicite | 4 passés, !91 / !92 / !383 et 430 demandes |
| `cargo build --locked --offline` | PASS |
| `cargo build --release --locked --offline` | PASS ; binaire optimisé disponible localement |
| Parcours sur le binaire optimisé | 4 passés : ascendance, sortie du rapport, workers, administration séparée |
| Python 3.11+, découverte `tests/test_*.py` | 68 recensés : 63 passés, 4 historiques sautés par défaut mais validés séparément, 1 saut EILSEQ décrit plus haut |
| Python, découverte `experiments/exclusion/test_*.py` | 6 passés, dont 5 issus des retours de session ; exclusion toujours NON_DEMONTREE |
| `scripts/sync-ai.py --check` | 12 fichiers synchronisés ; activation native UNKNOWN |

La régression indépendante des identités forge contradictoires passe pour ses
trois sous-cas : projet, HEAD, et les deux ensemble. Les faux objets intermédiaires,
blobs absents, positifs SHA-256 et protections des worktrees déplacés passent.
La CI distante n'a pas été exécutée.

La revue finale indépendante de C n'a trouvé aucun défaut bloquant concret dans
les raccordements de concurrence, gardes natives, changements de volumes,
contrôles de sortie globale et rescans via le principal. Elle ne remplace pas
les tests exécutés et ne lève aucune des limites FileProvider, TCC ou E1.

État Git à cette étape : 64 fichiers non suivis, sur `main` sans premier commit, aucun
autre statut. Les changements couvrent les dix modules produit, les fixtures
et tests, Cargo, la CI et la documentation. La source canonique des instructions
renvoie désormais à ce registre ; AGENTS/CLAUDE et le manifeste ont été régénérés.
Les définitions des agents restent inchangées. Le [changelog](../CHANGELOG.md)
recense les changements. À cette étape : aucun commit, push ou publication.

La clôture du 05/10, après les trois lots du RETEX et leurs corrections de revue,
est consignée dans [finish-ledger.md](finish-ledger.md) : 92 tests Rust et 78 tests
Python passent, ainsi que 6 témoins E1/RETEX et 5 parcours sur le binaire optimisé.
E1, U5 à U7 et la validation distante gardent les limites décrites plus haut.

Git interdit les transports avec une liste vide `GIT_ALLOW_PROTOCOL`, en plus
du refus des téléchargements implicites. Le test réel vérifie qu'une configuration
locale autorisant le protocole ne réactive pas son exécuteur. Référence primaire :
[documentation Git des variables de transport](https://git-scm.com/docs/git/2.53.0.html).


## Audit lisible : intégration du 07/10/2026

Le lot courant implémente `--html` sur `scan`, `review` et `plan`, le rendu
hors ligne par famille et le reçu de publication du HTML. Les étapes de
progression affichent des observations provisoires d’histoire et d’état local.
Le correctif d’index `intent-to-add` conserve la distinction index/disque.
Ce paragraphe décrit le checkout en cours ; le SHA livré et la validation
complète sont consignés à la fin du lot, séparément des passages ciblés.

Contrôles ciblés observés : 10 tests de rendu HTML, 9 de publication, 5 parcours
CLI HTML (dont multi-racines avec doublon, worktrees extérieurs enregistrés et
racine inaccessible), 8 tests de terminal/PTY. Les 2 régressions du formatage
progressif ont échoué avant implémentation puis réussi. Les reçus lient les
longueurs et hashes ; les oracles de fixtures ne constatent pas de mutation.

Chromium a ouvert un rapport synthétique de 2 familles et 4 worktrees à
1920, 1280, 390 et 320 px, avec et sans JavaScript. Aucun appel réseau, erreur
JavaScript ou débordement horizontal observé. Recherche, filtre de verdict,
tri, thème et détails au clavier vérifiés ; rendu contrôlé à zoom CSS 200 %.
Ce dernier contrôle ne prouve pas le comportement de tous les navigateurs au
zoom natif. Captures desktop/mobile relues visuellement. Une revue indépendante
a trouvé l’omission des lacunes de manifestes `scan --local` ; corrigée, testée
et relue. Les mesures de performance et la recette réelle ont leurs preuves
séparées ; ces tests n’établissent pas un inventaire actuel de la machine.


Le skill reste de portée dépôt : source `harness/skills/wt-sweep`, projections
`.claude/skills/wt-sweep` et `.agents/skills/wt-sweep`. `sync-ai.py --check`
valide 22 fichiers. Les caches BM25 de ce checkout ont été reconstruits pour
Claude et Codex, sans changement des hooks ni des réglages globaux. Chaque client
sélectionne le skill sur les 14 sondes positives et l’évite sur les 6 négatives
exécutées. Il est éligible et le seuil global passe ; F1 de calibration 0,966,
F1 de l’évaluation croisée 0,683. Ces sondes ne prouvent pas une absence générale
de faux positifs. L’invocation native dans une nouvelle session reste inconnue.

Le prompt « reprendre un rapport sans relancer l’analyse » a été rejoué par
un sous-agent sur le rapport synthétique de quatre lignes : trois `keep`, un
candidat. Le lecteur a vérifié les tailles et SHA-256 des trois artefacts
liés au reçu, cité les motifs et conditions de chaque ligne, puis proposé de
préserver la note locale et de confirmer l’usage du candidat. Aucun scan ni
retrait exécuté ; artefacts inchangés. Ce cas possède des tailles complètes et
aucun filtre : les cas partiels et filtrés relèvent des tests séparés. Ce rejeu
contrôlé ne démontre pas l’activation native du skill dans une nouvelle session.

### Vérifications locales du lot audit

Au checkpoint `bfdef510`, sur le code Rust et les assets HTML,
`cargo fmt --all -- --check` et
Clippy tous targets passent. La suite Rust complète compte 361 succès et
3 ignorés : benchmark volontaire, entrée enfant exécutée par son test parent et
import historique nécessitant des données explicitement choisies. La suite
Python complète compte 291 cas, 285 succès et 6 ignorés : quatre imports
historiques non demandés et deux créations de chemins refusées par le système
de fichiers (EILSEQ). Les cas d’identités en octets restent testés séparément.
Les six témoins d’exclusion passent ; ils démontrent les contre-exemples, pas
l’exclusion des écrivains.

Le SHA-256 du binaire debug est resté
`14183414de93e18921550ef01ed1e990ebb1438e828dba76f395122b41bb3a59`
avant et après la suite Python. La release des essais navigateur et des douze
scénarios comparatifs porte l’empreinte
`a622edf51eb32ef8fe58991703f9b404bbc67d1526bddaec9f4950206d6f26bf`.
La capsule de sources construite conserve Cargo, tous les fichiers Rust et les
assets CSS/JS. Les tests du protocole de benchmark ajoutés ensuite sont
consignés séparément. La CI du commit livré reste une preuve distincte.


Le [benchmark de collecte du 07/10](research/collection-benchmark-2026-10-07.md)
conserve huit échantillons propres comparables et deux échantillons avec fichiers
ignorés. Un couple standard à 100 worktrees reste explicitement non comparable.
Les six tests du protocole ajoutés après la suite Python complète passent
séparément ; ils vérifient notamment l’identité des preuves, la persistance des
mesures refusées et l’arrêt du groupe de processus possédé. Le test de signal
observe le vrai appel système mais ne constitue pas un oracle de disparition de
chaque descendant. Les deux tests du banc de comparaison des outils passent.


La [recette réelle multi-projets](research/multi-project-acceptance-2026-10-07.md)
a ensuite retrouvé les 69 inscriptions Git attendues : neuf `keep`, 60 `verify`,
aucun candidat. Les artefacts texte/JSON/HTML concordent ; le navigateur les lit
hors ligne. Les 69 résumés locaux restent partiels et seules dix mesures disque
sur 71 sont complètes. Le commit local `bfdef510` a été essayé ; deux pushes ont
été refusés pendant une panne GitHub avant la publication de la
[PR #15](https://github.com/FlorianBruniaux/wt-sweep/pull/15). La
[CI de `1e058f4`](https://github.com/FlorianBruniaux/wt-sweep/actions/runs/37644846382)
a ensuite réussi. Ce contrôle précède le correctif du premier fait visible
décrit dans le benchmark ; il ne valide pas ce correctif. Le SHA livré, sa CI
et la recette intégrée sont suivis dans
[#13](https://github.com/FlorianBruniaux/wt-sweep/issues/13). Aucun nettoyage réel exécuté.


### Correction du premier fait visible

La relecture du benchmark a distingué le premier statut provisoire du premier
fait collecté lisible. Les anciennes mesures conservaient parfois un statut
`unknown`/`partial` comme premier résultat ; elles ne prouvent pas ce second
délai. Le [registre de mesure](research/collection-benchmark-2026-10-07.md)
conserve ces essais et la campagne complémentaire avec ses propres binaires.

La sortie redirigée réserve désormais son jalon d’histoire à une relation connue
complète, et son jalon local à un compte exact ou une borne positive. Un événement
entièrement inconnu ne consomme plus ce jalon. Le terminal interactif et le mode
détaillé gardent les états provisoires. Le libellé d’un parcours commencé devient
`history comparison in progress; relation not established` ; le jugement final reste inchangé.

Trois tests de séquence passent après deux échecs attendus sur l’ancien gate.
Le test réel de progression passe, ainsi que sept tests du protocole et huit
scénarios console/PTY. La régression Git avec tranche d’une lecture a échoué
sur le libellé initial puis passé après correction. Formatage et Clippy tous
targets passent. La release est figée à
`e8a87860773e566b12f8696812591981101380d225f85de0944b4a87bd35f4e3`,
avec les 35 préimages source vérifiées avant et après construction puis relues
indépendamment. Les contrôles complets du SHA livré restent identifiés par le
lien de CI dans [#13](https://github.com/FlorianBruniaux/wt-sweep/issues/13).
