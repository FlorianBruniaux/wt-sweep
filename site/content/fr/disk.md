La taille disque aide à prioriser une revue. Elle ne participe pas au verdict de nettoyage.

## Périmètre et budgets

Ajoutez `--disk-usage` à `scan` ou `review`. Par défaut, toutes les racines de worktrees inventoriées et les dossiers Git partagés sont demandés. `--worktree` et les filtres de présentation ne réduisent pas cette collecte disque.

Utilisez `--disk-root CHEMIN_ABSOLU_EXACT`, répété si nécessaire, pour choisir les racines enregistrées. Incluez explicitement le Git partagé pour le mesurer. Un chemin inconnu est refusé.

`--disk-budget standard|thorough` est distinct du `--budget` Git/local. Commencez par standard ; des plafonds finis plus élevés peuvent encore laisser des lacunes. `plan --inventory` reprend les tailles capturées et leurs dates, sans les recompter.

## Lire les valeurs

| Valeur | Sens |
| --- | --- |
| Logique | Longueur des fichiers réguliers et liens. Un fichier creux peut dépasser largement son allocation. |
| Alloué | Blocs déclarés par le système de fichiers multipliés par 512, répertoires compris. |
| Git partagé | Administration Git commune, mesurée séparément. |
| Total dédupliqué | Chaque couple périphérique/inode observé compté une fois. Les lignes peuvent se recouvrir. |
| Complete | Cette racine est terminée sans lacune détectée. Ce n’est pas un instantané atomique. |
| Partial | Seul le cumul des observations est disponible. |
| Not requested | Racine enregistrée hors de la sélection disque explicite. |
| Not measured | Aucune taille disponible, pas zéro. |

Fichiers cachés et ignorés restent inclus. Les cibles des liens symboliques ne sont pas suivies. Les racines enregistrées imbriquées ont leurs propres lignes.

## Ce que ces tailles n’établissent pas

La taille allouée n’est pas l’espace récupérable. Clones APFS, snapshots et extents partagés peuvent changer le volume libéré. Alternates et dépendances externes aux racines mesurées ne sont pas comptés.

Le collecteur actuel utilise les lectures scalaires de métadonnées. L’évaluation native `getattrlistbulk` n’est pas intégrée. Aucune ventilation par catégorie de cache ni suppression automatique de caches.

[Choisissez une revue ciblée](/fr/docs/targets/) et conservez périmètres et dates de chaque exécution.
