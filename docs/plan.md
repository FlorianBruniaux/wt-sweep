# Plan de Worktree Sweep

Synthèse de conception consolidée le 04/10/2026. Les critères ci-dessous restent
la cible d'acceptation. Le [registre d'implémentation](progress.md) distingue les
comportements U1 à U4 testés localement des critères encore ouverts.
Les [retours des sessions de ménage](retex-sessions.md) précisent les prochains
lots complémentaires : rapport descriptif, inventaire local enrichi et identité
d'exécution. Ils ne modifient pas les conditions d'accès à U5 à U7.

## Choisir ce qu'il faut construire

Avant de figer le moteur d'application, comparer agentrinse aux mêmes fixtures :
worktree détaché, fichiers ignorés, index différent du disque, reflog seul,
écriture concurrente et restauration sans dépôt source. Comparer aussi les
scanners stalewood, gh-reaper et workroot aux cas de découverte d'U2.

Le résultat doit identifier chaque critère satisfait, refusé, manquant ou non
vérifié, à une révision précise. Décider ensuite entre contribution, adaptation
et implémentation de la fonction manquante. Une proximité du README ou un langage
différent ne suffisent pas à accepter ou rejeter un candidat.

## Livraisons et dépendances

| Unité | Résultat attendu | Dépendances |
|---|---|---|
| U1 | Inventaire d'une famille explicitement choisie, JSON et tableau | Socle |
| U2 | Découverte globale ou par racines, avec couverture observable | U1 |
| U3 | Rapport local explicable, forge inconnue autorisée | U1 |
| U4 | Ingestion d'exports forge et rapport combiné, fin du MVP | U2 et U3 |
| E1 | Démonstration d'exclusion des écrivains et contrat de métadonnées | Indépendante |
| U5 | Sauvegarde durable, restauration indépendante | U3 et E1 concluante |
| U6 | Plan exécutable et application sur fixtures jetables | U5 |
| U7 | Application réelle bornée, sélection exacte approuvée | U6 et revue indépendante |

Le chemin utile initial est U1 puis U3. La découverte U2 et la préparation des
exports forge peuvent avancer en parallèle. L'échec ou l'absence de preuve E1
laisse `apply` indisponible et ne bloque pas le rapport en lecture seule.

## Contrat du MVP

`scan` prend exactement une portée : `--repo`, `--root` répétable ou `--all`.
Le mode global parcourt le système de fichiers accessible et les volumes montés
visibles, y compris dossiers cachés, ignorés, caches et dépôts imbriqués. Il ne
s'arrête pas après avoir trouvé un premier dépôt. Les entrées `.git` peuvent être
des dossiers ou des fichiers ; les dépôts nus et administrations déplacées sont
recherchés par structure puis validés avec Git contrôlé.

Croiser la découverte sur disque et les enregistrements de chaque famille Git.
Garder une origine pour chaque observation, dédoublonner par identité et exposer
marqueurs cassés, désaccords, boucles de liens et administrations orphelines.
Ne pas réparer ni élaguer pendant la collecte. Inventorier sous-modules et dépôts
imbriqués ne les rend pas supprimables.

Le rapport garde volumes/racines tentés, exclusions choisies, refus d'accès,
limites, interruptions et intervalle du parcours. Distinguer parcours terminé,
lacunes et cohérence temporelle non garantie. Ne pas certifier un volume non
monté, une VM non exposée ou des données distantes non résidentes.

`scan` et `plan` n'écrivent pas dans les cibles. Une sortie `--out` est explicite,
hors des cibles et de leurs administrations. Ni fetch, ni prune, ni gc, ni
suppression de branche. Neutraliser les mécanismes Git exécutables et les
téléchargements implicites, ou refuser la lecture concernée. Erreurs et délais
dépassés ne deviennent pas des listes vides.

Le jugement déterministe sépare faits, risques et inconnues. Une MR ouverte reste
un signal d'activité prioritaire. Une observation d'intégration nomme sa relation,
sa cible et les OID concernés. La tête source d'une MR n'est pas sa cible.
Patch-id, diff vide, simulation de fusion, âge et MR fermée ne prouvent pas une
intégration. Un modèle ou classifieur éventuel donne des suggestions sans autorité.

`wt-sweep.review/1` reste un rapport sans actions. Les forges sont enrichies par
exports bruts et reçus d'exécution : périmètre, argv, dates, codes de sortie,
pages, curseurs et empreinte des octets persistés. Une pagination terminée ne
prouve pas un instantané cohérent. Une absence de MR reste inconnue sans preuve
de complétude et de cohérence ; préserver les faits positifs des exports partiels.

## Acceptation du rapport

- U1 : ensemble exact non vide contenant principal, lié, détaché et entrée
  manquante ; erreurs distinguées de l'absence. Témoins fsmonitor/filtres inactifs
  et manifeste avant/après inchangé, produit indépendamment du collecteur.
- U2 : `.git` fichiers et dossiers, bare à nom arbitraire, administrations
  séparées, ignorés, imbrications, accès refusé et annulation. Aucun doublon ni
  omission silencieuse ; résultats indépendants de la concurrence choisie.
- U3 : observations positives non vides et cible attendue ; rejet des faux
  positifs de patch-id, pilote de fusion, gitlink masqué et objet remplacé.
  Une implémentation qui classe toujours « à trier » doit échouer.
- U4 : export remplacé, page manquante, saturation, curseur répété, MR déplacée
  entre pages et versions contradictoires ne permettent aucune preuve d'absence.
  Un export complet, cohérent et non vide doit aussi passer.

## Conditions d'une application future

E1 doit exclure les écritures dans les fichiers, administrations et objets
pendant capture, retrait et vérification. Un résultat `lsof` vide, un verrou
entre instances, un second scan ou un renommage ne suffisent pas. Éprouver le
descripteur ouvert depuis un autre cwd, le commit détaché tardif et la modification
à taille/mtime identiques. Les [témoins E1](decisions/exclusion-et-conservation.md)
ont été exécutés : l'exclusion reste `NON_DEMONTREE`.
L'[examen des capacités macOS](decisions/apply-readiness.md) constate aussi le
refus du lease d'écriture et de la révocation d'un fichier régulier dans le
contexte testé. U5 ne commence pas sur la seule base d'un manifeste de fichiers.

U5 conserve octets suivis, non suivis, ignorés, fichiers masqués, index complet,
HEAD, refs propres, reflogs et objets nécessaires. Modes, liens, ACL, attributs
étendus et types spéciaux sont couverts et testés ou bloquants. Aucune exclusion
implicite fondée sur `node_modules` ou une copie dans le clone principal.
Une ancre ne remplace pas les objets. Restaurer dans une destination neuve sans
accès à la source ni aux alternates, puis comparer à un manifeste indépendant.
Vérifier la persistance des données, noms, répertoires et journal avant tout
retrait. Corruption, espace disque et erreurs de synchronisation doivent bloquer.

U6 introduit un schéma distinct `wt-sweep.apply-plan/1`. L'approbation humaine est
liée au digest complet, pas à un score ou à une empreinte présentée sans accord.
Sous exclusion E1, revalider, ancrer, sauvegarder, vérifier la durabilité, puis
retirer et vérifier le résultat. Aucun `--force` ni suppression de branche ;
le premier périmètre exclut sous-modules et dépôts imbriqués, y compris nus.
Un cas positif doit réellement retirer une fixture, laisser une sentinelle
inchangée et restaurer tout le contrat ailleurs. Injecter interruptions et
écrivains ; une implémentation qui refuse toujours échoue.

U7 exige une revue indépendante et une sélection approuvée. L'autorisation de
tests jetables n'autorise aucune suppression réelle. Le harness de développement
ne peut jamais franchir ces barrières à la place du produit ou de l'utilisateur.
