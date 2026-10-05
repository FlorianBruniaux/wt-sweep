# Tester Worktree Sweep, puis examiner un dépôt

[README français](../README.fr.md) · [Index documentaire](README.md) · [Référence CLI](cli.md)

Ce parcours permet de compiler l’outil, de lire un premier rapport sur un dépôt
jetable, puis de lancer une revue sur un dépôt choisi. Le
[prompt à copier](#prompt-à-donner-à-un-assistant) reprend ce dernier parcours.

Le MVP inventorie et produit des rapports. Il ne sauvegarde, ne restaure et ne
supprime aucun worktree. Un résultat `integration_observed` ne suffit pas à
supprimer un dossier contenant encore des fichiers locaux.

## 1. Préparer l’outil

Il faut macOS, `/usr/bin/git` et Rust. Python 3.11 ou plus récent est nécessaire
pour les tests automatiques, pas pour utiliser la CLI. Depuis un clone de
`wt-sweep`, lancez :

```sh
cargo fetch --locked
cargo build --release --locked --offline
./target/release/wt-sweep --version
./target/release/wt-sweep --help
```

`cargo fetch` télécharge les dépendances ; vous pouvez l’omettre si elles sont
déjà en cache. Rustup peut aussi télécharger la version fixée par le dépôt.
Pour obtenir les sources, suivez les
[instructions de clonage](../README.fr.md#compiler-depuis-les-sources).

La commande `--help` doit présenter `scan`, `plan` et `forge`, ainsi que
l’indisponibilité d’`apply`. Sans argument, l’outil affiche l’aide et ne lance
aucun scan.

## 2. Essayer sur un dépôt jetable

Copiez ce bloc depuis la racine de `wt-sweep`. Il crée un dépôt et un worktree
sous un nouveau dossier `/private/tmp/wt-sweep-tutorial.*`, avec les rapports.
La configuration Git est isolée pour la démo.

```sh
(
  set -eu
  sweep_bin="$PWD/target/release/wt-sweep"
  export GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_COUNT=0
  unset GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE
  unset GIT_OBJECT_DIRECTORY GIT_ALTERNATE_OBJECT_DIRECTORIES

  demo_dir="$(mktemp -d /private/tmp/wt-sweep-tutorial.XXXXXX)"
  mkdir -p "$demo_dir/reports/with-target" "$demo_dir/reports/without-target"
  git init --template= --initial-branch=main "$demo_dir/repo"
  git -C "$demo_dir/repo" -c user.name=Demo -c user.email=demo@example.invalid \
    -c commit.gpgSign=false commit --allow-empty -m "Demo baseline"
  git -C "$demo_dir/repo" worktree add -b demo "$demo_dir/worktree"
  printf 'Local note, absent from the commit\n' > "$demo_dir/worktree/notes.txt"
  initial_head="$(git -C "$demo_dir/repo" rev-parse HEAD)"

  "$sweep_bin" scan --repo "$demo_dir/repo" --local \
    --out "$demo_dir/reports/inventory.json"

  "$sweep_bin" plan --inventory "$demo_dir/reports/inventory.json" \
    --target refs/heads/main --summary --run-dir "$demo_dir/reports/with-target"

  "$sweep_bin" plan --inventory "$demo_dir/reports/inventory.json" \
    --summary --run-dir "$demo_dir/reports/without-target"

  apply_status=0
  (cd "$demo_dir/repo"; "$sweep_bin" apply) > "$demo_dir/reports/apply.stdout" \
    2> "$demo_dir/reports/apply.stderr" || apply_status=$?
  test "$apply_status" -eq 2

  printf 'Local note, absent from the commit\n' | cmp - "$demo_dir/worktree/notes.txt"
  test "$(git -C "$demo_dir/repo" rev-parse HEAD)" = "$initial_head"
  test "$(git -C "$demo_dir/worktree" rev-parse HEAD)" = "$initial_head"
  printf 'Note et HEAD inchangés ; apply a refusé avec le code 2.\n'
  printf 'Démo et rapports : %s\n' "$demo_dir"
)
```

Le premier rapport commence ainsi :

```text
Worktrees: 2
Active: 0; integration observed: 2; to triage: 0
Unknowns: 2 worktrees; coverage gaps: 0
```

Le second, sans cible d’intégration, commence ainsi :

```text
Worktrees: 2
Active: 0; integration observed: 0; to triage: 2
Unknowns: 2 worktrees; coverage gaps: 0
```

Les deux dernières lignes de chaque résumé donnent le chemin du rapport détaillé
et son empreinte. Ces valeurs changent à chaque exécution. Le dossier de la démo
est conservé pour que vous puissiez l’examiner.

### Ce qu’il faut vérifier

| Contrôle | Résultat attendu |
| --- | --- |
| Découverte | Deux worktrees, en comptant le principal. |
| Avec `--target refs/heads/main` | Deux `integration_observed` : les HEAD correspondent à la cible. |
| Sans `--target` | Deux `to_triage` : une référence non désignée comme cible ne prouve pas l’intégration. |
| Fichier local | `notes.txt` est toujours présent, avec son contenu initial. L’inventaire le classe `untracked`. |
| Refus d’application | Code `2` et diagnostic `apply is unavailable` dans `reports/apply.stderr`. |
| État Git | Les deux HEAD restent inchangés. |

Chaque sous-dossier `run-*` contient `report.txt`, `review.json` et `run.json`.
Ouvrez `report.txt` pour lire les relations et les inconnues. Dans
`review.json`, `local_inventories` contient l’entrée de `notes.txt` : catégorie
`untracked`, taille de 35 octets et empreinte SHA-256. Son contenu n’est pas
sauvegardé dans le rapport. L’[extrait JSON annoté](contracts.md#example-review-item)
montre comment retrouver cette entrée et son worktree.

Les inconnues et le statut `partial` des reçus sont attendus ici : aucun export
de forge n’a été fourni et `plan` ne relit pas les fichiers locaux inventoriés.
Un code de sortie `0` peut donc coexister avec des preuves incomplètes.

Le bloc contrôle la note et les HEAD. Il ne vérifie pas à lui seul la conservation
de tout le dépôt ; les tests automatiques ci-dessous utilisent des relevés
indépendants avant et après les commandes.

## 3. Lancer les tests automatiques

Depuis la racine du projet, après avoir téléchargé les dépendances :

```sh
(
  set -eu
  cargo test --locked --offline
  cargo build --locked --offline
  python3 -B -m unittest discover -s tests -p 'test_*.py' -v
)
```

La compilation sans `--release` prépare `target/debug/wt-sweep`, utilisé par les
tests Python. Le bloc s’arrête dès qu’une commande échoue. Les fixtures Git sont
créées par les tests ; ils ne lancent pas de découverte globale de votre machine.

Les suites couvrent notamment la découverte, les relations Git positives et les
faux positifs, les fichiers suivis/non suivis/ignorés, les imports de forge et les
sorties concurrentes. Les oracles Python comparent aussi des relevés de fichiers
et d’administration Git avant et après les appels. Les refus sont testés en même
temps que les cas qui doivent réussir.

**Réussite attendue :** aucun test en échec. Lisez séparément les tests marqués
`ignored` ou `skipped` : ils n’ont pas validé le comportement concerné sur votre
machine. Pour les contrôles de formatage, Clippy, les expériences d’exclusion et
la synchronisation du harness, utilisez la
[procédure complète de contribution](../CONTRIBUTING.md#set-up-and-verify).

Ces tests ne valident ni une suppression réelle, ni une sauvegarde/restauration,
ni un instantané de toute la machine. Les essais avec de vrais fichiers non
résidents FileProvider et de vrais refus TCC restent non vérifiés.

## 4. Examiner votre dépôt

Après la démo, choisissez un clone et une référence d’intégration existante.
Remplacez le chemin ci-dessous. Adaptez `refs/heads/main` si votre cible est, par
exemple, `refs/heads/master`.

```sh
repo_path="/absolute/path/to/your/clone"
report_dir="$(mktemp -d /private/tmp/wt-sweep-report.XXXXXX)"

./target/release/wt-sweep scan --repo "$repo_path" --local \
  --out "$report_dir/inventory.json"
./target/release/wt-sweep plan --inventory "$report_dir/inventory.json" \
  --target refs/heads/main --summary --run-dir "$report_dir"
```

Le dossier des rapports doit être hors du dépôt, de ses worktrees et de son
administration Git. Gardez un périmètre `--repo` pour ce premier essai : il inclut
les worktrees enregistrés du clone choisi. Le rapport dépend des références
locales présentes ; `refs/remotes/origin/main` n’est pas rafraîchie par un fetch.

Lisez les classifications avec les risques et inconnues. Si vous modifiez les
fichiers locaux, relancez `scan --local` vers une nouvelle sortie avant de
produire une nouvelle revue. `plan` ne réactualise pas leur contenu.

## Prompt à donner à un assistant

Ce prompt convient à un assistant disposant d’un terminal sur votre Mac, par
exemple Codex ou Claude Code. L’outil lui-même n’utilise aucun modèle. Remplacez
les trois valeurs entre chevrons avant de copier le bloc.

```text
Utilise Worktree Sweep pour examiner les worktrees de ce dépôt en lecture seule.

Sources de wt-sweep : <CHEMIN_ABSOLU_WT_SWEEP>
Dépôt à examiner : <CHEMIN_ABSOLU_DU_CLONE>
Cible d’intégration : <REFERENCE_COMPLETE, par exemple refs/heads/main>

Lis README.fr.md et docs/tutorial.fr.md dans les sources de wt-sweep. Vérifie que le binaire target/release/wt-sweep est disponible et que la cible indiquée existe localement. Si une valeur manque ou ne peut pas être vérifiée, explique le blocage sans en deviner une autre.

Lance scan --repo sur ce clone avec --local, puis plan sur l’inventaire obtenu avec la cible explicite, --summary et --run-dir. Crée un nouveau dossier de rapports hors du dépôt, de ses worktrees et de son administration Git. Ne réutilise ni n’écrase une sortie existante.

Reste sur ce dépôt et ses worktrees enregistrés. Ne lance ni scan --all, ni fetch, ni prune, ni gc, ni suppression de branche ou de worktree. N’appelle pas apply. Ne contacte pas GitHub ou GitLab pour enrichir le rapport ; sans export fourni, conserve l’état de la forge comme inconnu.

À la fin, donne-moi :
- les commandes réellement exécutées, leurs codes de sortie et les chemins des rapports ;
- un tableau par worktree : chemin, branche, HEAD complet, classification et relation Git observée vers la cible ;
- les fichiers locaux à examiner, les lacunes de couverture, les risques et les inconnues ;
- les points nécessitant une vérification humaine, sans proposition de suppression automatique.

N’interprète pas integration_observed comme une sauvegarde ou une autorisation de suppression. Rappelle que plan conserve les relevés locaux du scan sans relire les fichiers. Distingue toujours résultat observé, donnée manquante et vérification non exécutée.
```

Si l’assistant ne peut pas exécuter de commandes, lancez le parcours vous-même
et transmettez un extrait de rapport expurgé des données privées. L’encodage
hexadécimal des chemins dans le JSON est réversible : il ne les anonymise pas.

## Si le résultat diffère

| Symptôme | Vérification à faire |
| --- | --- |
| Dépendance introuvable en mode hors ligne | Relancer `cargo fetch --locked` avec accès réseau. |
| Garde macOS refusée | Conserver le diagnostic ; cette protection n’a pas d’option de contournement. |
| Un seul worktree dans la démo | Vérifier que la création du worktree a réussi et que le scan vise le dépôt jetable affiché. |
| Deux `to_triage` malgré la cible dans la démo | Lire les inconnues, vérifier la référence complète et la présence des objets Git. |
| Sortie déjà existante ou située dans le dépôt | Créer un nouveau dossier de rapports hors du périmètre inspecté. |
| Test en échec | Conserver le nom du test, le diagnostic, les versions de l’outil et de macOS, et la révision Git testée. |

Le [guide de dépannage](getting-started.md#troubleshooting) détaille les refus
de lecture et de publication. Pour signaler un problème, suivez les
[consignes de contribution](../CONTRIBUTING.md#report-a-problem).
