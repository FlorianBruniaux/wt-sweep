Après une [installation persistante](../../docs/installation.md), utilisez
`"$HOME/.local/bin/wt-sweep"` à la place du chemin de compilation. L’archive
alpha.4 inclut un installateur documenté par `bash install.sh --help`.

## Analyser un repo

Depuis le checkout wt-sweep compilé, choisissez le chemin absolu du repo et confirmez la ref d’intégration complète.

```sh
(
  set -eu
  sweep_bin="$PWD/target/release/wt-sweep"
  printf 'Absolute clone path: '
  IFS= read -r repo_path
  printf 'Confirmed full target ref: '
  IFS= read -r target_ref
  /usr/bin/git --no-optional-locks -c core.fsmonitor=false -C "$repo_path" \
    show-ref --verify --quiet "$target_ref"
  "$sweep_bin" review --repo "$repo_path" --target "$target_ref" \
    --disk-usage --summary --open
)
```

`review` découvre les worktrees et recueille les preuves sur leur état actuel en une exécution. `scan` enregistre un inventaire ; `plan --inventory` analyse un inventaire déjà enregistré. `review` n’accepte ni `--local`, ni `--inventory`.

## Analyse guidée depuis un inventaire

Disponible à partir d’alpha.4.
Dans un terminal, remplacez le chemin ci-dessous par votre inventaire enregistré :

```sh
wt-sweep plan --inventory /chemin/absolu/vers/inventory.json \
  --interactive --compare-local --group-by repo --summary --open
```

Choisissez les numéros des branches ou leurs refs complètes par dépôt, puis
confirmez le récapitulatif avec `yes`. `more` affiche la page suivante, `skip`
laisse l’intégration inconnue, `quit` annule. Rien n’est choisi par défaut.
Les tailles gardent leur intervalle d’origine ; les preuves courantes sont
relues. Pour automatiser sans terminal, gardez `--target-map`. Aucun fetch ni
nettoyage n’est exécuté.

## Fichiers produits

Avec `--html` ou `--open`, un nouveau dossier `run-*` contient `inventory.json`, `review.json`, `report.txt`, `report.html` et `run.json`. Gardez-les ensemble. Le dossier des rapports doit être en dehors de toutes les racines et familles Git inspectées. Ses répertoires parents doivent être des dossiers ordinaires.

Sur macOS, les rapports HTML vont par défaut dans `~/Desktop/Worktree Sweep/run-*/`. Le Bureau doit exister. Ajoutez `--run-dir "/dossier/absolu/existant"` pour choisir une sortie hors du périmètre inspecté. `--html` enregistre sans ouvrir ; `--open` demande aussi à macOS d’ouvrir le HTML après publication. Un échec d’ouverture laisse les fichiers disponibles et affiche un avertissement. La console fournit une commande pour rouvrir le rapport.

`scan --html` produit un inventaire marqué **not reviewed**, sans attribuer de verdict.

## Lire le récapitulatif console

Le bilan en anglais commence par l’allocation disque observée en KB/MB/GB, si elle a été mesurée. Il sépare les relations entre commits des verdicts de nettoyage, puis liste jusqu’à cinq worktrees sélectionnés par taille observée, leurs blocages et les limites de collecte. La commande de reprise reprend les cibles enregistrées pour la ligne ; vérifiez qu’elles conviennent encore. Une taille partielle peut sous-classer un worktree et l’allocation observée n’est pas l’espace récupérable.

## Options par besoin

| Besoin | Option |
| --- | --- |
| Un repo | `--repo CHEMIN_ABSOLU` |
| Racines explicites | `--root CHEMIN_ABSOLU` répété |
| Preuve d’intégration | `--target REF_COMPLÈTE` ou `--target-map` vérifiée par famille |
| Mesurer le disque | `--disk-usage`, budget indépendant `--disk-budget standard\|thorough` |
| Cibler les vérifications Git et locales coûteuses | `--worktree CHEMIN_EXACT`, `--budget standard\|thorough` |
| Cibler les racines disque | `--disk-root CHEMIN_EXACT` répété avec `--disk-usage` |
| Enregistrer et ouvrir le HTML | `--html`, `--open`, `--run-dir DOSSIER_EXISTANT` |
| Présentation | `--summary`, `--verdict`, `--path-prefix`, `--group-by` |

Les filtres d’affichage ne réduisent pas la collecte. Les guides [cibles et filtres](/fr/docs/targets/) et [périmètre disque](/fr/docs/disk/) expliquent la distinction.

La commande peut se terminer avec le code `0` alors que les preuves restent partielles. Lisez la couverture, les conditions manquantes et les diagnostics. La [référence CLI complète du repo](https://github.com/FlorianBruniaux/wt-sweep/blob/main/docs/cli.md) définit toutes les options et les codes de sortie.

## Comparer les changements locaux

Ajoutez `--compare-local` à la commande ci-dessus, en gardant `--open` ou `--html`
(ou un `--run-dir` explicite). Cette comparaison refuse `--out`.
`--compare-reference CHEMIN_ABSOLU` ajoute un autre checkout de la même famille
Git physique. L’auto-comparaison, ses alias et les familles ambiguës sont refusés.
Le fichier lié `local-comparison.json` conserve égalités, différences, inconnues
et couverture. Il ne change pas les verdicts CLI.

## Sélection des dépôts après alpha.4

Les sources après alpha.4 affichent `Repositories>` avant les branches.
Utilisez `find NOM_DU_PROJET` pour chercher dans les chemins enregistrés, puis
les numéros ou `all` pour les résultats. `worktrees` liste les familles avec
checkout ; `more` change de page et `reset` réaffiche tout. Aucun choix par défaut.
Seules les familles avec des cibles confirmées reçoivent une collecte courante.
Les autres lignes restent inconnues dans la source. Un seul dépôt choisi ouvre
une vue ciblée, sauf si `--filter-repo` est fourni. Les mesures disque gardent
leur périmètre d’origine.

Les scans sauvegardés séparent les compteurs de worktrees et de dépôts nus et
regroupent les avertissements. Les détails restent dans les fichiers et dans
la sortie `--verbose`.
