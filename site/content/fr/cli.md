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
  report_dir="$(mktemp -d /private/tmp/wt-sweep-report.XXXXXX)"
  "$sweep_bin" review --repo "$repo_path" --target "$target_ref" \
    --disk-usage --summary --html --run-dir "$report_dir"
  open "$report_dir"/run-*/report.html
)
```

`review` découvre les worktrees et recueille les preuves sur leur état actuel en une exécution. `scan` enregistre un inventaire ; `plan --inventory` analyse un inventaire déjà enregistré. `review` n’accepte ni `--local`, ni `--inventory`.

## Fichiers produits

Avec `--html --run-dir`, un nouveau dossier `run-*` contient `inventory.json`, `review.json`, `report.txt`, `report.html` et `run.json`. Gardez-les ensemble. Le dossier des rapports doit être en dehors de toutes les racines et familles Git inspectées. Ses répertoires parents doivent être des dossiers ordinaires.

`--html` exige `--run-dir`. `scan --html` produit un inventaire marqué **not reviewed**, sans attribuer de verdict.

## Options par besoin

| Besoin | Option |
| --- | --- |
| Un repo | `--repo CHEMIN_ABSOLU` |
| Racines explicites | `--root CHEMIN_ABSOLU` répété |
| Preuve d’intégration | `--target REF_COMPLÈTE` ou `--target-map` vérifiée par famille |
| Mesurer le disque | `--disk-usage`, budget indépendant `--disk-budget standard\|thorough` |
| Cibler les vérifications Git et locales coûteuses | `--worktree CHEMIN_EXACT`, `--budget standard\|thorough` |
| Cibler les racines disque | `--disk-root CHEMIN_EXACT` répété avec `--disk-usage` |
| Présentation | `--summary`, `--verdict`, `--path-prefix`, `--group-by` |

Les filtres d’affichage ne réduisent pas la collecte. Les guides [cibles et filtres](/fr/docs/targets/) et [périmètre disque](/fr/docs/disk/) expliquent la distinction.

La commande peut se terminer avec le code `0` alors que les preuves restent partielles. Lisez la couverture, les conditions manquantes et les diagnostics. La [référence CLI complète du repo](https://github.com/FlorianBruniaux/wt-sweep/blob/main/docs/cli.md) définit toutes les options et les codes de sortie.
