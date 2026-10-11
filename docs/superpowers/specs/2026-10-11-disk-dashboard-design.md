# Tableau de bord disque du rapport HTML

[Index documentaire](../../README.md) · [Contrats](../../contracts.md) · [Mesures disque](../../disk-usage.md)

Statut : spec approuvée le 11 octobre 2026. Le
[plan d'implémentation](../plans/2026-10-11-disk-dashboard.md) est exécuté
localement avec revue indépendante et corrections. Son bilan distingue les
contrôles réussis, les replays et les cas non vérifiés. Ces ajouts, postérieurs à l’archive `v0.1.0-alpha.1`, sont regroupés dans `v0.1.0-alpha.2`. Le [guide de release](../../release.md) définit la recette du nouveau paquet.

## Résultat attendu

L'utilisateur veut comprendre combien de worktrees ont été trouvés, consulter
leur liste immédiatement et situer leur taille par rapport à son disque.
Le rapport conserve les compteurs et la liste directement visible. Un tableau
de bord compact ajoute deux jauges par volume et la couverture des mesures.

Exemple synthétique : « 100 Go mesurés, soit environ 5 % d'un volume de 2 To ».
Ce pourcentage exprime un rapport entre allocation observée et capacité. Les
blocs partagés APFS empêchent d'en déduire une occupation physique exclusive
ou l'espace libérable après suppression.

## Présentation

Le résumé apparaît avant la liste, sans pousser celle-ci derrière un panneau
fermé. Les libellés suivent la langue du rapport existant.

| Indicateur | Information affichée |
| --- | --- |
| Worktrees trouvés | Dossiers présents, dépôts bare, chemins absents et non vérifiés, avec le nombre d'inscriptions Git en complément. |
| Espace du volume | Capacité, espace disponible pour l'utilisateur et jauge de la part non disponible. |
| Taille mesurée | Allocation dédupliquée des worktrees et du Git partagé sur ce volume, pourcentage de capacité, mention « mesure partielle » lorsque nécessaire. |
| Couverture | Mesures complètes, partielles, sans résultat et non demandées, séparées entre dossiers de travail et Git partagé. |
| Analyse de nettoyage | Avec une revue : nombres à conserver, candidats au nettoyage et à vérifier. Avec un inventaire seul : « Analyse de nettoyage non effectuée ». |

La couverture des mesures ne prouve pas que toute la machine a été découverte.
Une découverte interrompue reste signalée à côté des compteurs. Les inscriptions
Git et les racines mesurées sont deux populations différentes.

Chaque volume possède sa propre carte. La première est celle portant le plus
d'allocation observée ; les suivantes restent consultables sans JavaScript.
Aucune capacité ni quantité disponible n'est additionnée entre volumes.
Plusieurs volumes APFS peuvent partager le même conteneur et subir des quotas
ou réserves : [documentation Apple](https://support.apple.com/guide/disk-utility/add-delete-or-erase-apfs-volumes-dskua9e6a110/mac).
Le produit affiche la capacité rapportée par le système de fichiers, sans
prétendre identifier la capacité du SSD physique.

Une valeur inconnue est expliquée : « Capacité non enregistrée dans cet
inventaire » ou « Volume non identifié pour ces octets ». Aucun score global
ni seuil rouge de nettoyage n'est ajouté. Les filtres de la liste ne changent
pas ces chiffres source.

## Collecte et association au volume

Les informations sont capturées uniquement avec `--disk-usage`, pendant la
collecte existante. `plan` et le rendu HTML n'interrogent jamais la machine.
La capacité reste consultable après déplacement du rapport ou démontage.

L'approche retenue utilise `fstatfs` sur un descripteur de répertoire déjà
acquis par la mesure disque. L'identité device/inode du descripteur est vérifiée
avant et après l'observation. Cette association relie les compteurs de capacité
au device réellement mesuré ; ni le préfixe du chemin ni une conversion de
`fsid` en device ne constituent une preuve. Un échec ou une contradiction
conserve les octets mesurés avec une capacité inconnue.

Une observation est tentée par device, au plus 4 096 devices, dans le budget
restant de la collecte. Aucun montage supplémentaire n'est ouvert pour obtenir
des statistiques. Un device rencontré uniquement à travers des entrées sans
descripteur vérifié garde sa capacité inconnue. Les tentatives, erreurs et
omissions sont enregistrées. Les lectures utilisent la politique dataless
existante et une frontière native privée, avec les types du SDK fournis par
`libc`. Aucune dépendance ou commande externe n'est ajoutée.

Les compteurs natifs proviennent de `f_blocks`, `f_bfree`, `f_bavail` et
`f_bsize` : [statfs et fstatfs Apple](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/statfs.2.html).
Les durées et limites sont contrôlées avant et après chaque opération ; comme
les lectures de métadonnées existantes, un syscall bloqué dans le noyau n'a
pas de garantie d'interruption immédiate. Cette limite reste documentée.
Les capacités et disponibilités fluctuantes n'entrent pas dans la comparaison
de stabilité de la table des montages. Aucune photographie atomique n'est
annoncée.

L'allocation par device est incrémentée au point de comptage des inodes,
uniquement après insertion dans la déduplication globale `(device, inode)`.
Les racines imbriquées, liens physiques et montages traversés ne peuvent donc
pas être attribués en sommant les tailles des racines. La somme des totaux par
device doit égaler `disk_usage.unique_bytes`, pour les octets logiques comme
pour l'allocation. La collecte conserve son refus de traverser une frontière
de système de fichiers ; un autre volume est mesuré depuis ses propres racines
enregistrées. Les erreurs restent rattachées à leurs mesures source.

## Données enregistrées et calculs

`disk_usage` reçoit un bloc facultatif `storage`, versionné par
`schema: wt-sweep.storage/1`. Il contient les totaux par device et les
observations de volume : identité vérifiée, chemins en `BytePath`, type de
système de fichiers, capacité, espace libre, espace disponible, méthode,
intervalle de capture et raisons d'indisponibilité. Les champs de taille
absents restent inconnus ; ils ne valent jamais zéro.

La couverture des mesures est calculée depuis les lignes existantes, sans
inventer une couverture complète par device à partir d'une racine incomplète.
Le dashboard utilise conservativement l'incomplétude globale pour qualifier
les parts mesurées. Une omission des statistiques de capacité est diagnostique
et ne retire aucune mesure ni preuve Git.

Les calculs vérifient les produits et conversions :

```text
capacité = f_blocks × f_bsize
libre = f_bfree × f_bsize
disponible = f_bavail × f_bsize
non disponible = capacité - disponible
part mesurée = allocation unique du device / capacité
```

« Non disponible » inclut ce que le système n'offre pas à l'utilisateur ; ce
libellé ne signifie pas « autres fichiers ». Capacité nulle, compteurs invalides,
disponibilité supérieure à la capacité ou débordement rendent le calcul
concerné inconnu. Une part mesurée supérieure à 100 % conserve sa valeur
numérique et affiche un dépassement ; seule la longueur de la jauge est bornée.
Les pourcentages utilisent une arithmétique entière élargie et un arrondi
documenté à une décimale. Le rendu ne convertit pas les octets en nombres
JavaScript susceptibles de perdre de la précision.

## Compatibilité et limites

Le nouveau lecteur accepte les anciens inventaires sans `storage`. Un champ
absent n'est pas sérialisé, pour préserver leurs formes canoniques et digests.
Les anciens lecteurs stricts peuvent refuser les nouveaux artefacts enrichis ;
cette limite de compatibilité est documentée. Les versions des enveloppes
inventaire et revue restent inchangées pour cette extension facultative.

La revue et les vues filtrées conservent le bloc source entier. Publication
et budgets restent stricts : conserver les données ou refuser avec une erreur
explicite. La capacité ne modifie aucun verdict C1-C5.

Les rapports déjà capturés ne reçoivent pas rétroactivement les statistiques
actuelles de la machine. Ils affichent la taille enregistrée avec un pourcentage
indisponible. Obtenir ces jauges nécessite une nouvelle collecte explicitement
demandée avec son périmètre. Aucun scan global n'est lancé pendant le développement.

## Acceptation

- Deux racines qui se chevauchent et des liens physiques conservent une seule
  contribution globale ; un montage imbriqué attribue ses octets au bon device.
- Deux volumes APFS de capacité identique produisent deux cartes, jamais une
  capacité cumulée. Les tests injectent volumes et compteurs synthétiques.
- Échec natif, changement d'identité, limite atteinte, valeur nulle, débordement
  et ratio supérieur à 100 % ont chacun un rendu explicite et vérifiable.
- Le rendu d'un ancien artefact préserve son digest et sa liste ; le rendu
  d'un nouvel artefact fonctionne hors ligne sans aucune lecture système.
- Une revue conserve exactement les observations source. Les filtres et tris
  modifient la liste seule ; les jauges gardent leurs valeurs.
- Les jauges possèdent un texte accessible et fonctionnent dans les deux
  thèmes, sur mobile et sans JavaScript. La liste reste ouverte par défaut.
- La frontière native fait l'objet d'une revue indépendante avant livraison.

L'implémentation touchera le modèle et le collecteur disque, leur frontière
native, le rendu HTML et les tests correspondants. Les contrats, le guide
disque et la documentation des frontières natives seront mis à jour avec le
comportement réellement vérifié. L'exécution native avec revue indépendante
finale est autorisée le 11 octobre 2026.
