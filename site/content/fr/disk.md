La taille disque aide à choisir quels worktrees examiner en premier. Elle ne détermine pas le verdict de nettoyage.

## Périmètre et budgets

Ajoutez `--disk-usage` à `scan` ou `review`. Par défaut, wt-sweep demande la mesure de toutes les racines de worktrees recensées et des dossiers Git partagés. `--worktree` et les filtres de présentation ne réduisent pas cette collecte disque.

Utilisez `--disk-root CHEMIN_ABSOLU_EXACT`, répété si nécessaire, pour choisir les racines enregistrées. Incluez explicitement le Git partagé pour le mesurer. Un chemin inconnu est refusé.

`--disk-budget standard|thorough` est distinct du `--budget` des vérifications Git et locales. Commencez par standard ; des limites plus élevées ne garantissent pas une collecte complète. `plan --inventory` reprend les tailles capturées et leurs dates, sans les recompter.

Le récapitulatif console utilise KB/MB/GB et liste les cinq plus gros worktrees sélectionnés selon leur taille observée. Les mesures partielles peuvent les sous-classer ; les tailles absentes restent signalées.

## Lire les valeurs

| Valeur | Sens |
| --- | --- |
| Logique | Longueur des fichiers réguliers et liens. Un fichier creux peut dépasser largement son allocation. |
| Alloué | Blocs déclarés par le système de fichiers multipliés par 512, répertoires compris. |
| Git partagé | Dossier d’administration Git commun au repo et à ses worktrees, mesuré séparément. |
| Total dédupliqué | Chaque couple périphérique/inode observé compté une fois. Les lignes peuvent se recouvrir. |
| Complete | La mesure de cette racine est terminée sans omission détectée. Les fichiers n’ont pas tous été lus au même instant. |
| Partial | Seul le cumul des observations est disponible. |
| Not requested | Racine recensée mais non sélectionnée pour la mesure disque. |
| Not measured | Aucune taille disponible, pas zéro. |

Fichiers cachés et ignorés restent inclus. Les cibles des liens symboliques ne sont pas suivies. Les racines enregistrées imbriquées ont leurs propres lignes.

## Ce que ces tailles n’établissent pas

La taille allouée n’est pas l’espace récupérable. Clones APFS, snapshots et extents partagés peuvent changer le volume libéré. Alternates et dépendances externes aux racines mesurées ne sont pas comptés.

Le collecteur actuel lit les métadonnées entrée par entrée. L’[évaluation du 11 octobre](https://github.com/FlorianBruniaux/wt-sweep/blob/main/docs/research/native-disk-evaluation-2026-10-11.md) conserve cette voie scalaire dans le produit. Le prototype natif corrigé donne les mêmes totaux sur les fixtures et gagne sur les trois corpus fixes ; son adaptation aux protections du produit et aux frontières système reste non qualifiée. Ces timings C ne démontrent pas une accélération du produit Rust. `getattrlistbulk` n’est pas intégré. Aucune ventilation par catégorie de cache ni suppression automatique de caches.

[Choisissez une analyse ciblée](/fr/docs/targets/) et conservez le périmètre et les dates de chaque exécution.

## Capacité capturée des volumes

Depuis `v0.1.0-alpha.2`, les sources capturent les statistiques de volume depuis les descripteurs de répertoire vérifiés pendant `--disk-usage`. Le HTML affiche capacité, espace disponible et allocation mesurée par volume, avec une couverture distincte pour les dossiers de travail et le Git partagé. Les capacités des volumes APFS ne sont jamais additionnées. Le bloc facultatif `disk_usage.storage` reste disponible au rejeu hors ligne ; les anciens inventaires gardent leurs tailles sans pourcentage inventé. Les anciens lecteurs stricts peuvent refuser ces artefacts enrichis. Les verdicts restent inchangés.
