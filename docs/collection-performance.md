# Collecte des gros dépôts, 06/10/2026

[Index documentaire](README.md) · [Limites CLI](cli.md#resource-limits) · [Collecte ciblée](targets-and-filters.md#complete-the-evidence-for-one-worktree)

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
