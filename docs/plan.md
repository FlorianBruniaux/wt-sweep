# Plan de Worktree Sweep

[Index documentaire](README.md) · [Architecture](architecture.md) · [Référence CLI](cli.md)

Contrat initial U1-U7 et E1, consolidé le 04/10/2026. U1-U4, les ajouts
[RETEX](retex-sessions.md) et le [triage T01-T09](superpowers/plans/2026-10-05-worktree-triage-v2.md)
sont livrés. La [feuille de route](roadmap.md) indique le travail restant ; le
[registre de validation](finish-ledger.md#triage-t01-t09) distingue les preuves
locales, la livraison GitHub et les limites de l'essai réel.

Les sections du MVP ci-dessous conservent le contrat historique `review/1`.
Le contrat courant `review/2`, ses verdicts et C1-C5 sont définis dans les
[contrats des artefacts](contracts.md#triage-policy) et le plan T01-T09.
Les conditions E1 et U5-U7 restent ouvertes ; le triage ne les remplace pas.

## Incréments de lecture seule, livrés le 08/10/2026

Le [rapport HTML](html-report-plan.md) est livré par la PR #15,
sans nouvelle règle de décision. Ses critères portent sur la fidélité au
JSON, l’échappement, les vues filtrées, la limite de 32 MiB, les erreurs de
publication et le reçu. La progression expose des faits d’historique et
d’état local provisoires ; seuls le rapport final et ses preuves portent
la décision complète. Le résumé local doit conserver intent-to-add comme
travail non indexé, pour les index SHA-1 et SHA-256.

Les preuves locales sont dans le [registre](progress.md) ; le SHA livré,
sa CI et la recette intégrée sont suivis dans
[#13](https://github.com/FlorianBruniaux/wt-sweep/issues/13). Le [benchmark sur familles Git de test](../scripts/benchmark-review.py)
et la [comparaison runtime](research/worktree-runtime-comparison-2026-10-07.md)
conservent argv, identités des binaires, sorties et limites de preuve. Les
essais concurrents ne remplacent ni les contrôles Sweep encore attendus
ni E1/U5-U7.

La PR #18 a livré les finitions HTML, #22 la mesure disque ciblée et le partage
des plafonds, puis #23 les lectures Git groupées et la découverte reprenable.
L’[évaluation #20](research/native-disk-evaluation-2026-10-11.md) porte uniquement
sur le coût et les frontières du collecteur disque natif. La [recette du 9 octobre](research/real-family-acceptance-2026-10-09.md)
conserve les limites d’usage observées ; elle ne franchit aucune condition E1/U5-U7.

## Choisir ce qu'il faut construire

Avant de figer le moteur d'application, comparer agentrinse aux mêmes fixtures :
worktree détaché, fichiers ignorés, index différent du disque, reflog seul,
écriture concurrente et restauration sans repo source. Comparer aussi les
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
| E1 | Démonstration d'exclusion des processus d’écriture et contrat de métadonnées | Indépendante |
| U5 | Sauvegarde durable, restauration indépendante | U3 et E1 concluante |
| U6 | Plan exécutable et application sur fixtures temporaires | U5 |
| U7 | Application réelle bornée, sélection exacte approuvée | U6 et revue indépendante |

Le chemin utile initial est U1 puis U3. La découverte U2 et la préparation des
exports forge peuvent avancer en parallèle. L'échec ou l'absence de preuve E1
laisse `apply` indisponible et ne bloque pas le rapport en lecture seule.

## Contrat du MVP

`scan` prend exactement une portée : `--repo`, `--root` répétable ou `--all`.
Le mode global parcourt le système de fichiers accessible et les volumes montés
visibles, y compris dossiers cachés, ignorés, caches et repos imbriqués. Il ne
s'arrête pas après avoir trouvé un premier repo. Les entrées `.git` peuvent être
des dossiers ou des fichiers ; les repos bare et administrations déplacées sont
recherchés par structure puis validés avec Git contrôlé.

Croiser la découverte sur disque et les enregistrements de chaque famille Git.
Garder une origine pour chaque observation, dédoublonner par identité et exposer
marqueurs cassés, désaccords, boucles de liens et administrations orphelines.
Ne pas réparer ni élaguer pendant la collecte. Inventorier sous-modules et repos
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
le premier périmètre exclut sous-modules et repos imbriqués, y compris nus.
Un cas positif doit réellement retirer une fixture, laisser une sentinelle
inchangée et restaurer tout le contrat ailleurs. Injecter interruptions et
processus d’écriture ; une implémentation qui refuse toujours échoue.

U7 exige une revue indépendante et une sélection approuvée. L'autorisation de
tests sur des fixtures temporaires n'autorise aucune suppression réelle. Le harness de développement
ne peut jamais franchir ces barrières à la place du produit ou de l'utilisateur.
