Cette démo crée un repo et ses worktrees dans un dossier temporaire réservé à l’essai. Elle analyse uniquement ces fichiers et enregistre les rapports en dehors du repo. Lancez-la depuis le checkout wt-sweep compilé, sur macOS.

## Créer et analyser le repo de test

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

| Dossier de test | Verdict | Motif |
| --- | --- | --- |
| `repo` | `keep` | Checkout principal, point d’entrée du repo. |
| `worktree` | `keep` | `notes.txt` est une note locale non suivie. |
| `clean-worktree` | `cleanup_candidate` | Conditions C1–C5 actuelles établies sur une collecte complète. |

Les trois HEAD correspondent à `refs/heads/main`. La note locale reste à préserver. L’utilisation du worktree candidat n’a pas été vérifiée. Si la collecte est partielle, gardez les limites signalées et suivez les conditions indiquées plutôt que de chercher à obtenir ce verdict à tout prix.

## Parcourir un exemple déjà généré

[Ouvrir le rapport interactif](/demo/report.html). Cet autre repo de test contient quatre worktrees : trois `keep`, un `cleanup_candidate`. Son [relevé de génération](/demo/provenance.json) décrit l’origine du rapport.

Les captures de la page d’accueil datent du 8 octobre 2026 et proviennent d’un autre repo de test de quatre worktrees. Leur [relevé des captures](/assets/report-captures.json) est distinct de celui de la démo interactive.

Aucune des deux démos ne supprime de worktree. Le reçu de publication décrit les octets des fichiers produits ; il ne contient pas de sauvegarde.
