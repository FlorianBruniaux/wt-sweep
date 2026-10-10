Cet essai crée une nouvelle famille Git dans un répertoire temporaire possédé par la démo. Il touche uniquement cette fixture et sauvegarde ses rapports hors du dépôt. Exécutez-le depuis le checkout wt-sweep compilé, sur macOS.

## Créer et analyser la fixture

```sh
(
  set -eu
  export GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_COUNT=0
  unset GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE
  unset GIT_OBJECT_DIRECTORY GIT_ALTERNATE_OBJECT_DIRECTORIES

  demo_dir="$(mktemp -d /private/tmp/wt-sweep-demo.XXXXXX)"
  mkdir "$demo_dir/reports"
  git init --template= --initial-branch=main "$demo_dir/repo"
  printf 'Disposable worktree triage demo\n' > "$demo_dir/repo/README.txt"
  git -C "$demo_dir/repo" add README.txt
  git -C "$demo_dir/repo" -c user.name=Demo -c user.email=demo@example.invalid \
    -c commit.gpgSign=false commit -m "Demo baseline"
  git -C "$demo_dir/repo" worktree add -b demo "$demo_dir/worktree"
  git -C "$demo_dir/repo" worktree add -b clean-demo "$demo_dir/clean-worktree"
  printf 'Local note, absent from the commit\n' > "$demo_dir/worktree/notes.txt"

  ./target/release/wt-sweep review --repo "$demo_dir/repo" \
    --target refs/heads/main --disk-usage --summary --html --run-dir "$demo_dir/reports"
  printf 'Demo and reports: %s\n' "$demo_dir"
)
```

La commande affiche le dossier temporaire et les chemins des rapports. Ouvrez le `report.html` sauvegardé dans votre navigateur.

## Résultat attendu

| Fixture | Verdict | Motif |
| --- | --- | --- |
| `repo` | `keep` | Checkout principal, point d’entrée du dépôt. |
| `worktree` | `keep` | `notes.txt` est une note locale non suivie. |
| `clean-worktree` | `cleanup_candidate` | Conditions C1–C5 actuelles établies sur une collecte complète. |

Les trois HEAD correspondent à `refs/heads/main`. La note locale reste à préserver. L’usage du candidat n’a pas été vérifié. Si la collecte est partielle, conservez les lacunes et suivez les conditions indiquées plutôt que de forcer ce résultat attendu.

## Parcourir un exemple déjà généré

[Ouvrir le rapport natif interactif](/demo/report.html). Cette autre fixture du site contient quatre worktrees : trois `keep`, un `cleanup_candidate`. Le rapport provient d’un dépôt jetable, avec son [relevé de génération](/demo/provenance.json).

Les captures de la landing datent du 8 octobre 2026 et proviennent d’une autre fixture de quatre worktrees. Leur [relevé des captures](/assets/report-captures.json) est distinct de l’exemple fraîchement généré.

Aucun nettoyage dans les deux démos. Un reçu lie les octets des artefacts ; ce n’est pas une sauvegarde.
