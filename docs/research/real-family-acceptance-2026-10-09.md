# Recette réelle d’une famille Git, 9 octobre 2026

[Index documentaire](../README.md) · [État publié](../progress.md#état-publié-et-suite-au-09102026) · [Mesures disque](../disk-usage.md) · [Travail restant](../roadmap.md)

La campagne porte sur une seule famille réelle de 71 worktrees enregistrés.
Le premier passage de revue donne 10 `keep`, 61 `verify` et aucun candidat.
Deux reprises ciblées donnent `keep` pour les deux lignes examinées. La synthèse
du dernier verdict CLI par chemin est donc 12 / 59 / 0. Elle conserve les
rapports d’origine et leurs intervalles ; ce n’est pas un nouveau verdict de
Sweep ni un instantané de la famille.

## Provenance et périmètre

Le coordinateur a lancé les cinq commandes en lecture seule, sans délégation,
avec `wt-sweep 0.1.0` sur macOS. Le binaire figé porte le SHA-256
`a9bfd3e3f686a314712d9237ec9b4aa42e41e5e124b3ee76933b9f61047ca3fa`.
Son arbre source `51607716edce064809dbf4593f990851b7e28fab` est identique à
celui du commit main `bc054af0c75ae3299fa69e094ca9d6d33e813db2`, fusionné par #23.
Le numéro 0.1.0 désigne le paquet de développement, pas une release publiée.

Chaque commande utilise `--repo` sur cette famille, un répertoire de rapports
neuf, `--html` et `--format json`. Les revues utilisent une cible locale
explicitement confirmée, `--no-context` et aucun export de forge. Aucun fetch,
nettoyage, reset ou changement des sources n’est exécuté par cette campagne.
Deux inscriptions désignent des chemins absents ; les 71 inscriptions ne
prouvent donc pas l’existence de 71 dossiers sur disque. Aucun scan machine global.

Les chemins, branches, OID privés, commandes complètes, rapports et CSV restent
locaux. Les lettres A à E ci-dessous anonymisent les cinq worktrees prioritaires.
Ce document publie les observations agrégées, pas les preuves privées brutes.

## Cinq passages, cinq intervalles

| Passage | Sélection et budget | Durée observée | Résultat |
| --- | --- | ---: | --- |
| Disque familial | `scan --disk-usage`, budget disque standard | 111,2 s | 53 641 195 520 octets alloués dédupliqués ; 11 racines complètes sur 72. |
| Revue familiale | `review --budget standard` | 170,5 s | 10 `keep`, 61 `verify`, aucun candidat. |
| Revue A | `--worktree` et `--path-prefix` A, `--budget thorough`, `--disk-root` A | 53,5 s | `keep` ; mesure disque complète. |
| Revue C | Même ciblage pour C, `--budget thorough` | 114,1 s | `keep` ; première mesure disque partielle. |
| Disque B à E | Quatre `--disk-root`, `--disk-budget thorough` | 77,5 s | Les quatre racines demandées sont mesurées complètement. |

Les cinq codes de sortie valent 0 et les cinq reçus sont `partial` : succès de
publication ne signifie pas collecte complète. La revue `thorough` n’augmente
pas le budget disque, qui dispose de son option distincte. Les racines non
demandées restent visibles ; un sous-ensemble mesuré complètement ne rend pas
complet le total de la famille.

Les temps proviennent des journaux du lanceur sur une machine active. Les scopes,
budgets et travaux diffèrent ; il ne s’agit pas d’un benchmark avant/après.
La revue familiale atteint notamment ses limites d’histoire et de collecte
locale. Les reprises ciblées ne suppriment pas les plafonds de fichiers et d’octets.

## Tailles et verdicts des priorités

| Worktree | Allocation observée | Taille complète | Dernier verdict original |
| --- | ---: | --- | --- |
| A | 10 372 456 448 octets (10,37 Go) | Oui | `keep`, ignorés et HEAD non ancêtre de la cible. |
| B | 7 546 957 824 octets (7,55 Go) | Oui | `verify`, revue familiale uniquement. |
| C | 5 484 564 480 octets (5,48 Go) | Oui, au dernier passage disque | `keep`, fichiers ignorés observés. |
| D | 5 457 346 560 octets (5,46 Go) | Oui | `verify`, revue familiale uniquement. |
| E | 5 484 367 872 octets (5,48 Go) | Oui | `verify`, revue familiale uniquement. |

Ces cinq dossiers ne sont pas un classement exhaustif des plus gros worktrees.
Les mesures ciblées ne sont pas ajoutées aux 53,64 Go partiels du premier scan :
elles ont des intervalles distincts et peuvent partager des inodes. Les tailles
allouées ne prédisent pas l’espace récupérable sur APFS.

Pour A, le graphe complet ne retrouve pas HEAD dans l’ascendance de la cible.
Cela n’exclut pas une intégration par squash, qui n’est pas établie. Au moins
7 442 fichiers ignorés sont observés ; les autres comptes partiels ne prouvent
pas leur absence. C contient au moins 25 598 ignorés. Sa relation de graphe
`ancestor` ne satisfait pas à elle seule C2, faute de vérification profonde
complète. Les conditions C2, C3 et C5 restent manquantes pour ces deux lignes ;
leur conservation repose sur des faits positifs.

## Cache volumineux : diagnostic distinct

Un diagnostic complémentaire de métadonnées trouve dans A 22 fichiers réguliers
sous `.next/cache`, pour 9 963 364 352 octets alloués (9,96 Go), dédupliqués par
périphérique/inode. Les douze plus gros fichiers sont confirmés ignorés par Git.
Le diagnostic ne lit pas leurs contenus et exclut répertoires et liens de cette
ventilation. Ce n’est pas une fonction de catégorisation livrée par Sweep.

Ces observations orientent vers l’examen du cache, en conservant le worktree et
ses fichiers locaux. Elles ne prouvent ni l’absence de build actif, ni les octets
qu’un effacement libérerait. Aucun cache ni worktree n’a été supprimé.

## Vérifications et limites

Les 18 artefacts des cinq runs correspondent aux tailles et SHA-256 de leurs
reçus. Le CSV local contient 71 chemins uniques ; les liens du bilan local
résolvent et les cinq mesures prioritaires sont complètes. La synthèse conserve
les verdicts CLI originaux, les identités physiques contrôlées et les sources
de chaque ligne. Le rendu navigateur HTML n’a pas été revérifié lors de cette
campagne ; l’accès aux fichiers locaux a été refusé par le navigateur disponible.

Une campagne préalable sur deux racines explicites de Sweep a comparé une
découverte reprise à un parcours neuf : familles, HEAD et identités identiques,
avec deux intervalles conservés pour la reprise. Ses douze artefacts ont aussi
été vérifiés. Ce résultat porte sur ces racines, pas sur toute la machine.

L’usage par les agents ou terminaux, l’exclusion des écrivains, une sauvegarde
durable et une restauration indépendante ne sont pas établis. Les stashes
restent une propriété de la famille ; un contexte vide ne prouve pas leur absence.
Les 59 inscriptions à vérifier appellent des revues ciblées, sans changement des
règles C1-C5. L’optimisation native de #20 reste à comparer sur corpus fixe avant
d’en mesurer l’effet sur une nouvelle campagne réelle.
