Une cible est une ref locale complète de la branche contre laquelle vérifier l’intégration. Aucune branche par défaut n’est devinée et aucun fetch n’est effectué.

## Une famille

Utilisez `--repo` et `--target refs/heads/...` ou `--target refs/remotes/...` après avoir confirmé la ref voulue. Une ref de suivi distant représente uniquement sa valeur enregistrée localement.

## Plusieurs familles

Choisissez des racines explicites et examinez l’inventaire. Une `--target-map` lie les refs complètes aux chemins d’administration et identités périphérique/inode enregistrés. Ne réutilisez pas une ancienne identité après son changement dans un nouvel inventaire. Une liste de cibles vide laisse l’intégration inconnue.

La [procédure de création du fichier de cibles](https://github.com/FlorianBruniaux/wt-sweep/blob/main/docs/targets-and-filters.md) conserve les octets des chemins et refuse les sorties existantes.

## Choix guidé dans le terminal

Les sources après alpha.3 proposent `plan --inventory FILE --interactive --open`.
La CLI affiche les branches locales par famille, accepte les numéros ou refs
complètes, puis demande `yes` après le récapitulatif. `skip` laisse l’intégration
inconnue sans masquer la famille. Rien n’est choisi par défaut. Gardez
`--target-map` pour automatiser sans terminal. [Commande guidée](/fr/docs/cli/#analyse-guidée-depuis-un-inventaire).

## Collecter moins ou afficher moins

| Contrôle | Effet |
| --- | --- |
| `--worktree CHEMIN_EXACT` | Cible les vérifications Git et locales coûteuses sur un worktree enregistré ; les autres lignes restent présentes. |
| `--disk-root CHEMIN_EXACT` | Cible séparément les racines disque demandées. |
| `--path-prefix CHEMIN`, `--verdict`, `--filter-repo` | Sélectionne les lignes affichées après collecte. |

Un filtre de présentation n’économise pas la collecte. Même une sélection vide conserve la couverture et les omissions de l’analyse complète.

## Reprise ciblée

Lisez d’abord les limites signalées dans le rapport. Si le budget standard a été atteint, choisissez un nouveau dossier de sortie et relancez l’analyse du même worktree avec `--budget thorough`. Les limites augmentent, mais la collecte reste bornée : ce budget ne résout ni les refus d’accès, ni les objets absents, ni un changement d’identité. Les mesures disque ont leur propre `--disk-budget`.

Lisez le résultat ciblé avant d’élargir le périmètre. Chaque exécution conserve ses dates et ses preuves. Vérifier si un agent ou un terminal utilise encore le worktree reste un contrôle distinct.
