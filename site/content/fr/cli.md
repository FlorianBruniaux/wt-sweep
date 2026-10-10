## Analyser un dépôt

Depuis le checkout source compilé, choisissez un chemin absolu et une ref d’intégration complète, confirmée.

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

`review` découvre et collecte des preuves actuelles en une exécution. `scan` sauvegarde un inventaire ; `plan --inventory` analyse un inventaire sauvegardé. `review` n’accepte ni `--local`, ni `--inventory`.

## Artefacts sauvegardés

Avec `--html --run-dir`, un nouveau dossier `run-*` contient `inventory.json`, `review.json`, `report.txt`, `report.html` et `run.json`. Gardez-les ensemble. La sortie doit être hors de toutes les racines et familles Git inspectées, avec des ancêtres répertoires ordinaires.

`--html` exige `--run-dir`. `scan --html` produit un inventaire **not reviewed**, sans verdict inventé.

## Options par besoin

| Besoin | Option |
| --- | --- |
| Un dépôt | `--repo CHEMIN_ABSOLU` |
| Racines explicites | `--root CHEMIN_ABSOLU` répété |
| Preuve d’intégration | `--target REF_COMPLÈTE` ou `--target-map` vérifiée par famille |
| Mesurer le disque | `--disk-usage`, budget indépendant `--disk-budget standard\|thorough` |
| Cibler les preuves Git/locales coûteuses | `--worktree CHEMIN_EXACT`, `--budget standard\|thorough` |
| Cibler les racines disque | `--disk-root CHEMIN_EXACT` répété avec `--disk-usage` |
| Présentation | `--summary`, `--verdict`, `--path-prefix`, `--group-by` |

Les filtres d’affichage ne réduisent pas la collecte. Les guides [cibles et filtres](/fr/docs/targets/) et [périmètre disque](/fr/docs/disk/) expliquent la distinction.

Un code `0` peut encore contenir des preuves partielles. Lisez couverture, conditions manquantes et diagnostics. La [référence CLI complète du dépôt](https://github.com/FlorianBruniaux/wt-sweep/blob/main/docs/cli.md) définit toutes les options et les codes de sortie.
