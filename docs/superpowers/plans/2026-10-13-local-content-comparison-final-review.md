# Revue indépendante de la comparaison locale

Le 13 octobre 2026, `comparison_final_review` a examiné le diff de
l'implémentation contre les préimages du début de session. Le checkout principal
contenait déjà des changements. Le paquet de revue isolait 29 fichiers de cette
implémentation, à HEAD `ec40d8f9ef1daf0b2c893551e1b37da91fd11d16`.

Le verdict du reviewer sur ce snapshot est **corrections nécessaires** : trois
findings Important, un Minor, aucun Critical étayé. La revue était statique et
indépendante de l'implémentation. Le reviewer n'a exécuté aucune fixture.

| Finding | Effet observé dans le code examiné | Régression demandée |
| --- | --- | --- |
| F1, Important | Le refus compare les chemins bruts. Une référence `/linked/.` peut comparer le worktree à lui-même. | Fichier unique non suivi : refuser l'alias physique, sans publier de run. |
| F2, Important | Le contrôle final vérifie le dossier racine, sans conserver le marqueur `.git` et la famille. Une réécriture en place peut laisser une référence déclarée stable. | Rediriger `.git` vers une autre famille après la capture initiale : invalider les preuves de cette référence. |
| F3, Important | La détection d'imbrication recherche seulement `.git`. Les fichiers d'un dépôt bare peuvent être pris pour une copie ordinaire. | Même `cache/config` dans la source et dans un bare imbriqué : résultat inconnu et couverture incomplète. |
| F4, Minor | Une absence source ne conserve pas son témoin de parent dans le sidecar. | Suppression également absente de la cible : conserver le parent ; refuser un sidecar qui le retire. |

Emplacements du snapshot examiné : `comparison.rs:509,541` pour F1,
`summary.rs:1874` pour F2, `summary.rs:1639` pour F3 et
`summary.rs:985,1917` pour F4. Les lignes actuelles ont changé après correction.

Le reviewer a constaté la présence des comptes liés à la source, des inconnues,
des couches index/disque, des digests et de la publication avant reçu. Le diff
examiné ne modifie pas directement C1-C5 ni la priorité de conservation.

Le reviewer a refusé de juger la réussite de la suite complète encore en cours,
le rendu navigateur, le comportement concurrent effectivement reproduit, les
performances réelles et l'activation native du harness.

L'intégrateur a reproduit les quatre défauts, puis réalisé un seul passage de
correction. La [recette](../../research/local-comparison-acceptance-2026-10-13.md)
enregistre les résultats après correction. Il n'y a pas eu de seconde revue
indépendante ; les vérifications des corrections sont celles de l'intégrateur.
