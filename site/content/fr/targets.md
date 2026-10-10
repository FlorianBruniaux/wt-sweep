Une cible est une ref locale complète de la branche contre laquelle vérifier l’intégration. Aucune branche par défaut n’est devinée et aucun fetch n’est effectué.

## Une famille

Utilisez `--repo` et `--target refs/heads/...` ou `--target refs/remotes/...` après avoir confirmé la ref voulue. Une ref de suivi distant représente uniquement sa valeur enregistrée localement.

## Plusieurs familles

Choisissez des racines explicites et examinez l’inventaire. Une `--target-map` lie les refs complètes aux chemins d’administration et identités périphérique/inode enregistrés. Ne réutilisez pas une ancienne identité après son changement dans un nouvel inventaire. Une liste de cibles vide laisse l’intégration inconnue.

La [recette canonique de carte des cibles](https://github.com/FlorianBruniaux/wt-sweep/blob/main/docs/targets-and-filters.md) conserve les octets des chemins et refuse les sorties existantes.

## Collecter moins ou afficher moins

| Contrôle | Effet |
| --- | --- |
| `--worktree CHEMIN_EXACT` | Cible les preuves Git/locales coûteuses sur un worktree enregistré ; les autres lignes restent présentes. |
| `--disk-root CHEMIN_EXACT` | Cible séparément les racines disque demandées. |
| `--path-prefix CHEMIN`, `--verdict`, `--filter-repo` | Sélectionne les lignes affichées après collecte. |

Un filtre de présentation n’économise pas la collecte. Même une sélection vide garde couverture et lacunes source.

## Reprise ciblée

Lisez d’abord les lacunes sauvegardées. Si les plafonds standard sont en cause, choisissez une nouvelle sortie et reprenez le même worktree exact avec `--budget thorough`. Les plafonds finis augmentent ; cela ne résout ni refus d’accès, ni objets absents, ni identité modifiée. Le disque possède son `--disk-budget` indépendant.

Lisez le résultat ciblé avant d’élargir le périmètre. Chaque exécution garde dates et preuves propres. L’usage par agents et terminaux reste une vérification distincte.
