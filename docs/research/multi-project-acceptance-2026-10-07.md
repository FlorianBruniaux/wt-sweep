# Recette multi-projets du 07/10/2026

Deux racines de projets explicitement choisies ont été analysées en lecture seule
sur le commit local `bfdef510928115ec2d2984d6aae8ea8061ef2ff7`. Les deux commandes
ont retourné 0. Les [preuves résumées](../../tests/data/acceptance/multi-project-2026-10-07.json)
conservent les intervalles, tailles et empreintes des artefacts sans publier les
chemins, branches de travail ni contenus privés. Les rapports complets restent locaux.

La publication était bloquée à ce point : GitHub a refusé deux pushes pendant
son [incident Git, Pull Requests et Actions](https://www.githubstatus.com/incidents/djlmxz2zd0j7).
Cette recette prouve le comportement du code local identifié. La CI distante et
la recette sur le SHA intégré restent des vérifications distinctes de [#13](https://github.com/FlorianBruniaux/wt-sweep/issues/13).

## Résultats

| Observation | Résultat |
| --- | --- |
| Familles et inscriptions connues | 2 familles, 67 et 2 worktrees ; les 69 chemins correspondent au relevé Git indépendant. |
| Verdicts | 9 `keep`, 60 `verify`, aucun candidat au nettoyage. |
| État local | 69 résumés collectés, tous partiels. Les raisons positives de conservation restent valables malgré les contrôles manquants. |
| Découverte | Plafond de dossiers atteint ; 1 248 lacunes conservées. Retrouver les inscriptions connues ne prouve pas l’absence d’un autre repo sous les racines. |
| Disque | 10 mesures complètes sur 71 racines, worktrees et Git partagé compris ; plafond global d’entrées atteint. |
| Octets observés | 35,43 Gio alloués et 30,90 Gio logiques, total dédupliqué par inode mais incomplet. Aucun chiffre d’espace récupérable. |
| Durée des commandes | `scan` avec disque et HTML : 168,947 s ; `plan` standard avec HTML : 208,927 s. Mesures descriptives sur des sources actives. |

Les phases locale et privée atteignent respectivement leurs plafonds de
120 et 60 secondes. Les refs sont les refs locales choisies par projet ; aucun
fetch n’a été exécuté. Leur ancienneté peut différer de celle des branches distantes.
Les raisons de `keep` comprennent modifications indexées ou non indexées,
fichiers ignorés, HEAD non intégré, opérations Git et checkouts principaux.
Ces motifs se recouvrent : leur somme ne donne pas le nombre de worktrees.

## Concordance et lecture

Les longueurs et SHA-256 de chaque artefact correspondent à leur reçu. Le digest
sémantique du JSON est vérifié indépendamment. Les 69 lignes HTML correspondent
aux chemins et verdicts JSON ; les comptes du texte sont identiques. `plan`
conserve exactement les observations disque capturées par `scan`.

Chromium affiche le rapport réel à 1280 et 390 px, avec et sans JavaScript :
69 lignes, aucun appel réseau, aucune erreur JavaScript et aucun débordement
horizontal observé. Détails au clavier, recherche, filtre, tri et thème vérifiés.
Les scénarios filtrés et les autres tailles d’écran ont leurs tests synthétiques
séparés. Aucun constat de conservation atomique n’est fait sur les repos réels,
qui peuvent être modifiés par d’autres sessions pendant la lecture.

Le [tutoriel](../tutorial.fr.md) fournit les commandes de premier passage,
le [guide des cibles](../targets-and-filters.md#several-repositories) décrit la
carte par famille et le [skill](../assistant-skill.md) donne le prompt de lecture.
Ce passage n’autorise aucun retrait et ne constitue pas un scan de la machine entière.
