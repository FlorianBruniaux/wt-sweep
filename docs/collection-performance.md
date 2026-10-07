# Collecte des gros dépôts, 06/10/2026

[Index documentaire](README.md) · [Limites CLI](cli.md#resource-limits) · [Collecte ciblée](targets-and-filters.md#complete-the-evidence-for-one-worktree)

## Indicateurs de performance

Le bilan de `scan` affiche le temps de découverte, les tentatives de lecture de
dossiers, le nombre d’entrées listées, le débit moyen et les workers demandés.
Ces mesures sont aussi conservées dans `coverage.discovery_metrics`. Le temps
vient de `std::time::Instant` ; aucun crate supplémentaire n’est nécessaire.
Les entrées listées comprennent fichiers et dossiers. Les worktrees enregistrés
hors racine peuvent être trouvés sans parcourir leur contenu.

`review --summary` ajoute le débit de la phase locale et ses appels Git, lorsque
ces compteurs sont présents. La phase la plus longue apparaît avec sa part du
temps de collecte enregistré. La découverte et la publication restent hors de
ce total. Avec `plan`, les mesures de découverte viennent de l’inventaire source.

Lire la couverture à côté du débit : une collecte qui atteint son plafond peut
être rapide et incomplète. Un compteur absent reste absent. Le nombre de chemins
connus en attente ne permet pas de calculer un pourcentage d’avancement fiable.
Le plafond de découverte reste de 60 secondes ; le profil `thorough` ne le change
pas. Aucun point de reprise du parcours n’est ajouté par ces mesures.

## Benchmark Rust reproductible

Depuis le checkout, exécuter le test facultatif en mode release :

```sh
cargo test --release --locked --offline --test discover benchmark_discovery -- --ignored --exact --nocapture
```

Le test crée une arborescence privée sous `/private/tmp`, avec deux familles Git,
quatre worktrees, un dépôt caché et 2 048 fichiers synthétiques répartis dans
64 dossiers. Il compare 1 et 4 workers avec un échauffement chacun, puis trois
mesures par configuration en alternant leur ordre. Il affiche minimum, médiane,
maximum, débit et ratio des durées médianes. Il vérifie à chaque passage les
familles et chemins exacts, la couverture complète, les mêmes unités de travail
et la conservation des octets, modes, identités et mtime de la fixture.

La génération et la vérification de la fixture sont hors chronométrage. La mesure
couvre l’appel Rust de découverte, pas le démarrage de la CLI ni la publication.
Le cache du système n’est pas vidé. Exécuter ce benchmark seul, hors suites de
tests concurrentes, et comparer des runs sur le même hôte et le même corpus.
Noter le commit et le diff local avec les résultats. Aucun seuil de durée absolu
ne fait échouer la CI ; un parcours incomplet ou une conservation violée échoue.

Le premier levier disponible est `--workers 1..8`, pour les métadonnées. Les appels
Git restent séquentiels. Si les processus Git dominent la phase locale, réduire
leurs appels ou mieux les grouper demande une mesure distincte sur cette phase.
Augmenter `--budget` autorise davantage de travail ; cela ne rend pas une lecture
plus rapide. Toute optimisation doit conserver les résultats et la couverture
pour que la comparaison ait un sens.

### Mesure locale du 06/10/2026

Benchmark exécuté en release sur macOS aarch64, après la fin des suites Rust et
Python. Chaque passage a visité 107 dossiers et listé 2 218 entrées, Git inclus.
Les deux familles et quatre worktrees attendus sont présents ; couverture
complète et manifeste de conservation inchangé pour les huit passages.

| Workers | Minimum | Médiane | Maximum | Débit à la médiane |
| --- | --- | --- | --- | --- |
| 1 | 0,925 s | 1,043 s | 1,306 s | 2 125,7 entrées/s |
| 4 | 0,871 s | 1,157 s | 1,222 s | 1 917,4 entrées/s |

Le ratio des durées médianes est de 1,11 pour 4 contre 1 worker. Les plages se
chevauchent : ce passage ne démontre aucun gain du parallélisme. Ce corpus local
ne mesure ni une machine entière, ni les performances de l’analyse des fichiers
locaux d’un gros dépôt. Le cache système et les autres applications ne sont pas
contrôlés ; trois échantillons ne suffisent pas à établir une distribution de
latence générale.

Identités de cette mesure : base `98a9a4bc0520a16f5759cbaff54127631a353ba2`,
branche locale `codex/scan-performance` avec changements non commités.
SHA-256 du diff `src/` et `tests/` :
`778e7b6079ee8c9942dc71ef01fbd7fcd5f882e5df842e8af46e815178589d94`.
SHA-256 de l’exécutable de benchmark :
`1cdb8d3ca677bb2ab14ef219a7f293b12b98bef739c373d6c08d264983f601d1`.
La sortie locale est conservée dans `/private/tmp/wt-sweep-perf-benchmark.log` ;
les mesures ci-dessus restent dans ce registre si le dossier temporaire disparaît.

## Premier fait utile sur 50 et 100 worktrees

La [mesure du 07/10/2026](research/collection-benchmark-2026-10-07.md)
compare deux binaires release figés, avec sorties redirigée et PTY séparées.
Le premier statut d’histoire complet arrive après 3,4 à 3,5 secondes sur le
corpus de 50 worktrees en standard, puis 5,6 à 6 secondes sur celui de 100
worktrees en thorough. Ce statut reste provisoire ; les verdicts attendent
les vérifications finales. Les preuves, plafonds et états des fixtures sont
identiques à l’intérieur de chaque couple retenu.

Un couple à 100 worktrees en standard a été refusé comme benchmark comparable :
les délais locaux et privés avaient produit des preuves différentes. Le rapport
conserve ce résultat, dont un temps total après reste inconnu. Le passage en
thorough utilise le profil public existant ; aucun plafond produit n’a changé.

La sonde séparée de 400 fichiers ignorés conserve 50 worktrees et leurs raisons
de `keep`. Elle mesure les lectures locales, mais ne se compare pas au temps
total du corpus propre : les worktrees retenus évitent les vérifications
approfondies réservées aux candidats. Les échantillons uniques et le cache OS
non contrôlé ne démontrent pas un gain général de vitesse.

## Profils et mesure d’une nouvelle collecte

`review` et `plan` acceptent `--budget standard|thorough`. Standard conserve les
plafonds d’origine ; thorough augmente les budgets locaux et ceux des
graphes tout en gardant vérification des OID, identité, revalidation et limites finies. Aucun
profil ne retire les fichiers ignorés de l’analyse. Consultez les
[plafonds comparés](cli.md#thorough-allowances).

Commencer par un run standard et inspecter ses faits. Si un plafond explique les
preuves manquantes, choisir une [collecte ciblée](targets-and-filters.md#complete-the-evidence-for-one-worktree)
thorough dans une nouvelle sortie, puis lire son résultat avant de choisir toute
la famille. Un nouveau run observe un autre intervalle ; il ne complète pas
rétroactivement l’ancien.

L’optionnel `collection_metrics` consigne profil, `applied_limits` et phases
`name`, `elapsed_ms`, `counters`. Les compteurs d’objets, d’octets et de processus
ne sont pas interchangeables. Les phases enregistrées incluent le contexte facultatif lorsqu’il est activé,
et excluent découverte et publication ; leurs temps ne valent pas durée totale
de commande.
Un plafond définit le travail autorisé, pas une garantie de débit ou de complétude.
L’absence de métriques dans un ancien rapport ne signifie pas zéro travail.

## Checkpoint A1-A6 : fixtures et recette réelle

Ce checkpoint distingue les positifs synthétiques, les deux passages réels
ciblés, le bilan de la famille complète et la sonde de transport à données
constantes. La recette réelle est terminée en lecture seule, avec ses preuves
partielles conservées. Aucun de ces essais n’autorise une suppression.

### Positifs synthétiques A1-A5

La fixture de 4 200 blobs distincts, 600 commits et 32 racines privées obtient
`cleanup_candidate` avec le profil thorough. HEAD, index et disque sont complets,
les cinq compteurs locaux sont complets à zéro, et toutes les racines privées
sont couvertes par des refs nommées conservées. La relation approfondie est
`ancestor`. Le manifeste indépendant de conservation reste inchangé.

L’oracle impose au moins 4 200 lectures approfondies d’objets et moins de
2 100 processus Git pour cette phase. Ces prédicats passent ; les compteurs
exacts par phase n’ont pas été persistés dans la sortie de cet oracle. Ils ne
constituent pas un benchmark de durée. La famille synthétique de 50 worktrees
obtient 48 candidats et deux `keep`, avec inventaire et manifeste de conservation
inchangés ; le worktree lié contenant 4 200 fichiers non suivis reste retenu.

Un témoin de racine directement couverte conserve un coût identique avec une ou
33 refs nommées : un objet, zéro étape de graphe et 16 processus Git. Le plafond
initial de quatre processus était une assertion d’oracle incorrecte, corrigée
après lecture du coût des contrôles et revalidations. Ce témoin ne prouve pas
qu’une histoire privée arbitraire se résout en une lecture.

Le binaire de cette capsule synthétique porte le SHA-256
`defd5e17c94dd8e330b3e76eafe8e239bf020b1d647152ab7e83f17266826a1c`.
Cette identité ne vaut pas validation d’un autre binaire construit ensuite.

### Premier essai réel thorough ciblé

Le run conserve 50 lignes source : trois `keep`, 47 `verify`, aucun candidat.
Un seul worktree est sélectionné pour les preuves coûteuses. Son ascendance
est vérifiée et ses deux racines privées sont couvertes. La collecte locale
expire avant l’examen des premiers chemins : `paths_checked` vaut zéro.
L’énumération et la classification du disque avaient commencé. HEAD,
index et disque restent incomplets ; les cinq zéros locaux sont incomplets,
donc ne prouvent pas un worktree propre. C2, C3 et C5 restent manquantes.

| Phase de ce passage | Temps enregistré | Compteurs observés |
| --- | --- | --- |
| Histoire du graphe | 9 938 ms | 376 lectures, 376 étapes, 377 processus Git |
| État local | 120 028 ms | 15 471 lectures, 138 702 567 octets, 260 processus Git, 26 712 réutilisations en cache, aucun chemin disque examiné |
| Histoire privée | 240 473 ms | 5 124 lectures, 5 091 étapes, 5 139 processus Git, deux racines énumérées |

Les tailles et empreintes des artefacts ont été vérifiées contre le reçu
`partial`. Le run établit des preuves positives d’histoire, puis un arrêt local
explicite ; il n’établit pas une collecte réelle complète. Les intervalles, le
contexte et les preuves non sélectionnées restent distincts.

### Second passage réel thorough ciblé

Après les ajustements des ancres privées et du transport Git, le run retourne
le code 0 avec un reçu `partial` vérifié. Il conserve 50 lignes : quatre `keep`,
46 `verify`, aucun candidat. Le worktree sélectionné devient `keep` pour une
modification non indexée observée et au moins 33 741 fichiers ignorés. Les
52 213 chemins examinés apportent des faits locaux utiles ; ils ne constituent
pas un inventaire complet. L’ascendance et les deux racines privées couvertes
restent établies ; C2, C3 et C5 restent manquantes.

| Phase de ce passage | Temps enregistré | Compteurs observés |
| --- | --- | --- |
| Histoire du graphe | 11 659 ms | 376 lectures, 376 étapes, 377 processus Git |
| État local | 108 840 ms | 15 471 lectures, 138 702 567 octets d’objets, 3 882 processus Git, 36 680 réutilisations en cache, 52 213 chemins examinés |
| Histoire privée | 5 063 ms | 234 lectures, 194 étapes, 249 processus Git, deux racines couvertes |

Le résumé local atteint le plafond cumulé de 512 MiB. Les octets d’objets du
compteur ne décrivent pas tout ce budget local, qui comprend les autres lectures
comptabilisées. Le diagnostic secondaire générique de délai ne démontre pas
une expiration des 120 secondes : la phase enregistrée dure 108 840 ms. Les
revalidations locales restent incomplètes et le détail est tronqué. Les
compteurs locaux incomplets sont des bornes minimales, pas des totaux exhaustifs.

L’histoire privée avait enregistré 240 473 ms et 5 124 lectures au premier
passage, contre 5 063 ms et 234 lectures dans ce second run. Ces observations
portent sur un dépôt mutable ; elles ne sont pas un benchmark avant/après à
données constantes et n’isolent pas l’effet d’une seule correction.
Le binaire de ce second passage porte le SHA-256
`e6f3b15d7de9ac2c68ca697d5e645615cc54cef258deb192997b52a4883019a9`.

### Ajustements et sonde de transport

La priorité des ancres privées déjà utiles utilise un quantum de 32 expansions.
Sur sa fixture, le journal indépendant des enfants Git observe trois lectures
natives de commits après correction, contre 67 avant, avec les deux racines
couvertes. Une chaîne dépassant le quantum conserve le repli vers les autres
ancres ; une borne insuffisante laisse les résultats inconnus.

Le transport remplace la pause fixe après une rafale par une attente bornée des
descripteurs prêts. Sur la même sonde synthétique optimisée, 16 enfants Git
retournent 64 MiB : 46 540 ms avec les pauses, 500 ms avec l’attente de disponibilité.
Cette sonde conserve les requêtes, l’exécutable Git, l’environnement et les octets
attendus. Elle mesure ce transport ; elle ne mesure pas la collecte du dépôt
réel. La sonde de 10 000 fichiers ignorés vides ne montre pas de gain
d’énumération : 967 ms avant, 1 587 ms après.

La régression de transport vérifie les octets d’un aller-retour de 32 MiB sous
le délai produit de dix secondes. Elle échoue avec la pause réintroduite puis
passe avec l’attente de disponibilité. Les contrôles ciblés couvrent aussi entrée
partielle, plafond stdout/stderr, statut d’échec et pipes retenus par un descendant.
La synthèse de la famille ci-dessous et les suites intégrées restent des preuves
distinctes de ces mesures ciblées.

### Recette thorough de la famille complète

La revue de la famille termine avec le code 0 : 50 worktrees, 49 `keep`, aucun
candidat et un `verify`. Les tailles et SHA-256 de tous les artefacts correspondent
au reçu `partial`. Les 50 résumés locaux sont collectés : dix sont complets,
40 partiels, aucun absent. Les relations du graphe sont connues pour les 50
lignes : 37 `not_ancestor`, 12 `ancestor`, une `identical_commit`.
Ces relations ne remplacent pas les preuves approfondies C2.

Les raisons de conservation se recouvrent : 49 lignes avec fichiers ignorés,
37 avec HEAD non intégré, huit avec contenu non suivi, huit avec modifications
non indexées, quatre avec index modifié, deux avec opération Git et une avec
checkout principal. Il ne faut pas additionner ces nombres. Les 49 `keep`
établissent des raisons de conserver ; ils ne signifient pas 49 analyses
exhaustives. Les comptes locaux partiels restent des bornes minimales.

| Phase de la famille | Temps enregistré | Compteurs agrégés |
| --- | --- | --- |
| Histoire du graphe | 119 375 ms | 4 954 lectures, 4 917 étapes, 4 955 processus Git |
| État local | 900 022 ms | 749 352 lectures, 6 543 436 425 octets d’objets, 21 330 processus Git, 1 046 267 chemins examinés |
| Histoire privée | 300 206 ms | 13 309 lectures, 13 290 étapes, 13 812 processus Git, 1 175 racines énumérées |

Les compteurs locaux agrègent plusieurs worktrees ; le plafond d’objets et
d’octets du profil s’applique à chaque worktree. Les phases locale et privée
atteignent leurs budgets de 900 et 300 secondes. L’histoire privée garde
aussi ses omissions et revalidations incomplètes. Le contexte activé est
chronométré séparément ; découverte et publication restent hors des temps de
phase. Le run n’est pas un instantané atomique du dépôt vivant, ni un benchmark
contrôlé à données constantes.

L’unique `verify` a une ascendance observée, mais son index contient une entrée
`intent-to-add` dont les sémantiques ne sont pas prises en charge. Son résumé
local reste incomplet ; l’histoire privée conserve un plafond de sortie atteint
et l’arrêt de phase. C2, C3, C4 et C5 restent manquantes. Ses cinq zéros locaux
incomplets ne prouvent pas qu’il est propre. Cette ligne demande une vérification
explicite de l’index et des preuves privées, sans retrait automatique.

Le SHA-256 du binaire figé a été relu sur ses octets :
`e6f3b15d7de9ac2c68ca697d5e645615cc54cef258deb192997b52a4883019a9`.
Le correctif final des diagnostics d’annulation et de plafond d’octets a été
relu sans constat ouvert. Les suites finales passent : 324 tests Rust réussis,
deux ignorés ; 271 tests Python réussis, six ignorés. Le
[registre de validation](finish-ledger.md#complétion-des-gros-dépôts-a1-a6)
identifie aussi le binaire release construit après ces derniers correctifs.
Ces comptes ne remplacent ni les preuves du run ni les limites de conservation.

## Checkpoint antérieur aux profils de budget

Les mesures, résultats de tests et digests ci-dessous décrivent le lot de
collecte précédent. Ils ne valident pas les nouveaux plafonds thorough.

## Défauts reproduits

Le rapport fourni de 50 worktrees avait quatre résumés locaux partiels et 46
non collectés. Chaque premier résumé consommait environ 30 secondes. L’histoire
comptait 23 980 étapes pour 556 lectures d’objets ; 46 comparaisons restaient
incomplètes. La découverte avait terminé sans lacune, ce qui ne suffisait pas
à établir les conditions C1-C5.

Les tests de volume antérieurs réutilisaient le même blob sur 25 001 chemins.
Deux nouvelles fixtures jetables ont reproduit le coût des contenus distincts :

| Fixture | Avant | Après |
| --- | --- | --- |
| 1 024 blobs distincts, arbre plat | 596 processus Git lancés avant le délai local ; résumé incomplet | 23 processus Git au total ; HEAD/index/disque complets, puis changements staged et unstaged correctement distingués |
| 1 024 dossiers et arbres distincts | 489 lectures d’objets avant le délai local ; résumé incomplet | Au plus 40 processus de lecture d’objets ; les trois volets sont complets |
| Graphe de 12 commits, sept worktrees | Une ascendance réelle restait inconnue sous le budget d’un seul parcours | 12 expansions, 14 lectures de commits, sept premiers passages, aucun résultat de graphe incomplet |

Les nombres de processus viennent du journal des enfants réellement lancés.
Les fixtures utilisent de vrais objets Git. Les vérifications SHA-1 et SHA-256,
les objets absents/corrompus, les lots dépassant 8 Mio et les plafonds restent
couverts. Ces comptages ne mesurent pas le temps sur une autre machine.

## Changement de collecte

Les arbres et blobs locaux sont lus par lots finis, après lecture bornée de
leurs tailles. Chaque objet reste vérifié contre son OID complet. Les lectures
Git gardent environnement neutralisé, répertoire ancré, statut de sortie,
plafond d’octets, délai et arrêt des enfants. Un objet qui remplit exactement
l’ancien plafond de sortie conserve une lecture scalaire lorsqu’il ne tient
pas dans le protocole de lot.

Le graphe d’une cible est partagé entre les worktrees et refs alias de la même
famille physique. Un nœud est atteint après vérification de ses octets et de sa
structure. Une preuve négative exige un parcours terminé sans erreur ; chaque
ref reçoit encore sa propre revalidation finale.

Le budget local restant est partagé entre les worktrees présents restant à
traiter, avec le même plafond individuel. `--worktree` concentre un nouveau
passage sur un chemin exact. Toutes les lignes et l’inventaire source restent
publiés, y compris les preuves volontairement non collectées ailleurs.

## Revue indépendante

Une revue statique séparée a trouvé un défaut de diagnostic : des lignes non
sélectionnées pouvaient être comptées comme sélectionnées après un changement
d’identité et recevoir un faux motif de budget épuisé. Deux régressions ont
reproduit ces erreurs, puis sont passées après correction. La relecture du
correctif n’a signalé aucun autre défaut actionnable. Elle n’a exécuté aucun
test indépendamment.

Une seconde revue statique a examiné les délais raccourcis d’histoire et
d’histoire privée. Les tests Git/FIFO ont reproduit leur classification erronée
en panne du backend. Après correction, une échéance de phase préserve les preuves
indépendantes ; un vrai timeout Git conserve la quarantaine. Une revalidation
privée manquante invalide aussi la complétude d’énumération, donc C4 et C5 restent
inconnues. La revue de ce delta n’a signalé aucun défaut actionnable.

## Mesure réelle et validation intégrée

Le premier essai après regroupement des lectures a découvert 49 worktrees :
6 `keep`, aucun candidat, 43 `verify`. Les 49 résumés locaux ont été tentés,
mais tous restent partiels sous leur part du budget global. Le nombre d’expansions
d’histoire est passé à 512, avec 555 lectures d’objets ; cela atteint encore le
plafond du graphe. Ce résultat ne démontre pas une analyse complète du dépôt.

Cet essai a révélé la fausse quarantaine liée à l’échéance d’histoire privée,
corrigée ensuite. Le dépôt avait changé depuis le rapport fourni de 50 worktrees.
Les durées et les nombres de verdicts ne constituent donc pas un benchmark
avant/après à données constantes. Les plafonds restent inchangés et `apply`
indisponible.

### Passage ciblé après correction des échéances

Un passage sur un worktree lié conserve les 49 lignes de l’inventaire, avec
48 omissions de collecte explicitement demandées. Il vérifie l’ascendance du
HEAD vers la cible `develop` : 358 expansions, 358 lectures de commits et une
comparaison d’histoire complète. Le résumé local observe au moins 4 969 fichiers
ignorés. La décision est `keep` pour cette raison positive, même si C2-C5 restent
incomplètes.

Le contrôle local atteint le plafond de 4 096 objets et tronque ses diagnostics ;
il ne constitue pas un inventaire exhaustif des fichiers. L’histoire privée
atteint encore son budget. La preuve d’ascendance survit à cette échéance, sans
être confondue avec la vérification approfondie des arbres et blobs nécessaire à
C2. Ce passage montre l’utilité d’une collecte ciblée, pas la complétude d’un gros
dépôt ni l’inactivité du worktree.

Le processus termine avec le code 0. Les tailles et SHA-256 des trois artefacts
correspondent au reçu. Les rapports détaillés restent dans le dossier temporaire
privé de la session ; aucun chemin personnel ni contenu local n’est copié ici.
Binaire essayé : SHA-256
`24aeefc93556fb2a83cf41c8c46d4cdf22b903dcc5ba0d522f69f82b61274721`.

### Nouveau passage du dépôt complet

Après correction des échéances, la revue de la même famille découvre encore
49 worktrees : 4 `keep`, aucun candidat, 45 `verify`. Les 49 résumés locaux sont
partiels, aucun n’est omis faute de temps avant son tour. Trois ascendances sont
vérifiées ; 46 comparaisons restent inconnues au plafond de 512 expansions.
La phase utilise 556 lectures de commits. Aucune ligne n’est mise en quarantaine
pour timeout du backend.

Cinq lignes n’établissent pas C1. Les 49 lignes restent incomplètes sur C2-C5 :
le graphe seul ne remplace pas les vérifications approfondies, les résumés locaux
atteignent leur délai et l’histoire privée son budget. Le passage ciblé reste
nécessaire pour obtenir les observations locales utiles à un worktree précis.
L’état du dépôt évolue entre passages ; ni les comptes de rétention ni les durées
ne mesurent un gain à données constantes. Des tests de fixtures tournaient aussi
sur la machine pendant ces essais.

Le processus termine avec le code 0 et les hashes/tailles des artefacts ont été
vérifiés contre le reçu. Binaire essayé : SHA-256
`1086ffd7e5a4baa9034e7fe532ab2182e88de1deb582f12709fa73f1825e30df`.
Les derniers ajustements de conseil console sont couverts séparément par les
neuf tests de présentation ; ils ne changent ni la collecte ni les verdicts.

## Validation locale du lot

- Suite Rust complète : 303 tests réussis, 2 ignorés, avec deux tests simultanés.
- Suite Python complète : 266 cas exécutés, 260 réussis et 6 ignorés selon les
  capacités du système de fichiers. Les scénarios à 46 puis 100 worktrees et
  leurs inventaires inversés passent.
- Six expériences de conservation réussies. Les neuf tests du bilan console
  passent aussi après le dernier ajustement du conseil ciblé.
- Formatage, Clippy avec avertissements refusés, builds debug/release, aide CLI
  et synchronisation des 22 fichiers d’instructions réussis.
- Contrôle documentaire : 31 fichiers, 327 liens locaux et 38 blocs shell.

Les résumés de tests ci-dessus couvrent leurs fixtures et les observations
consignées. Ils ne démontrent ni activation native du skill, ni suppression,
ni réussite de la CI distante. La compilation de livraison porte le SHA-256
`a6fc99ad9094bea4e9d3f1287c6291e414334ad7408af6057f04417480c22cbc`.
