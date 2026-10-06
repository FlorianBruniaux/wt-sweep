# Tester Worktree Sweep, puis examiner un dépôt

[README français](../README.fr.md) · [Index documentaire](README.md) · [Référence CLI](cli.md)

`review` inventorie le périmètre choisi et examine ses worktrees en une commande.
`keep` donne une raison de conserver, `cleanup_candidate` signifie que les
conditions actuelles C1-C5 sont établies, et `verify` indique les preuves
manquantes. Un candidat exige encore une vérification d’usage. Aucun verdict
ne sauvegarde les fichiers ni n’autorise leur suppression ; `apply` reste
indisponible.

## 1. Préparer l’outil

Il faut macOS, `/usr/bin/git` et Rust. Python 3.11 ou plus récent sert aux tests,
pas à la CLI. Depuis la racine d’un clone de `wt-sweep` :

```sh
cargo fetch --locked
cargo build --release --locked --offline
./target/release/wt-sweep --version
./target/release/wt-sweep review --help
```

`cargo fetch` télécharge les dépendances ; omettez-le si elles sont en cache.
Rustup peut aussi télécharger la version fixée par le dépôt. Pour obtenir les
sources, suivez les [instructions de clonage](../README.fr.md#compiler-depuis-les-sources).
L’aide, y compris `scan --help`, `plan --help` et `forge --help`, ne lance aucune
collecte. Sans argument, l’outil affiche l’aide.

## 2. Examiner votre dépôt en une commande

Copiez le bloc depuis la racine de `wt-sweep` après compilation. Il demande un
chemin absolu et une référence complète que vous avez confirmée. Il s’arrête si
la référence manque localement, sans en choisir une autre.

```sh
(
  set -eu
  sweep_bin="$PWD/target/release/wt-sweep"
  printf 'Chemin absolu du clone : '
  IFS= read -r repo_path
  printf 'Référence complète confirmée (refs/heads/... ou refs/remotes/...) : '
  IFS= read -r target_ref
  /usr/bin/git --no-optional-locks -c core.fsmonitor=false -C "$repo_path" \
    show-ref --verify --quiet "$target_ref"
  report_dir="$(mktemp -d /private/tmp/wt-sweep-report.XXXXXX)"
  "$sweep_bin" review --repo "$repo_path" --target "$target_ref" \
    --summary --run-dir "$report_dir"
  printf 'Rapports : %s\n' "$report_dir"
)
```

Les six `X` de `XXXXXX` sont à garder littéralement. `mktemp` les remplace par un
suffixe unique. Le dossier des rapports doit se trouver hors du clone, de ses
worktrees enregistrés et de leurs administrations Git. `--repo` inclut les
worktrees enregistrés de cette famille, même hors du dossier du clone.

Une référence `refs/remotes/origin/main` désigne sa valeur déjà présente
localement ; aucun fetch ne la rafraîchit. Le nom de la branche courante ne
confirme pas la cible d’intégration. Sans cible acceptée, un worktree propre
peut rester `verify`, tandis qu’une raison positive de conserver reste `keep`.

Chaque nouveau dossier `run-*` contient :

| Fichier | Ce qu’il conserve |
| --- | --- |
| `inventory.json` | L’inventaire original de cette découverte, sans compactage préalable au jugement. |
| `review.json` | Les preuves, décisions, intervalles et lacunes, ou une vue contenant la source complète. |
| `report.txt` | Le détail des lignes choisies, avec totaux et lacunes source. |
| `run.json` | Le reçu et les hashes des trois artefacts. |

Le bilan console distingue verdicts, couverture et contrôles restants.
Il ne remplace pas le détail. Le chemin d’exécution est
affiché lors de la publication. Un reçu lie les octets qu’il énumère ; une erreur
de synchronisation finale laisse la durabilité inconnue. Aucun fichier de cette
liste ne constitue une sauvegarde des worktrees.

### Lire chaque verdict

Le bloc précédent garde ses variables dans un sous-shell. Pour ces commandes,
renseignez dans votre shell le chemin et la référence que vous avez vérifiés :

```sh
repo_path="/absolute/path/to/your/clone"
target_ref="refs/heads/your-confirmed-target"
./target/release/wt-sweep review --repo "$repo_path" --target "$target_ref" --verdict keep
./target/release/wt-sweep review --repo "$repo_path" --target "$target_ref" --verdict cleanup_candidate
./target/release/wt-sweep review --repo "$repo_path" --target "$target_ref" --verdict verify
```

Ces valeurs sont des exemples à remplacer. Chaque commande collecte une nouvelle
revue puis filtre l’affichage ; elle conserve totaux et lacunes du périmètre
source. Pour relire les preuves déjà sauvées, ouvrez le `report.txt` de ce run.

Lisez `decision.keep_reasons`, `decision.missing_conditions`, le résumé
`local_summary` et l’histoire privée. `0` est exact pour un compteur complet,
`>=N` est une borne minimale et `UNKNOWN` désigne un zéro incomplet.
Les stashes appartiennent à la famille, pas au seul worktree où ils sont affichés.
Sujets de commits, changelog et PR/MR donnent du contexte sans établir
l’intégration. Une ascendance du graphe seule, une identité d’arbre ou une MR
fermée ne satisfait pas les [conditions C1-C5](contracts.md#triage-policy).

## 3. Essayer les trois verdicts sur un dépôt jetable

Ce bloc crée une fixture sous `/private/tmp`, avec deux worktrees liés. La
configuration Git est isolée. Il conserve la fixture et ses rapports pour lecture.

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
  printf 'Disposable worktree triage demo\n' > "$demo_dir/repo/README.txt"
  git -C "$demo_dir/repo" add README.txt
  git -C "$demo_dir/repo" -c user.name=Demo -c user.email=demo@example.invalid \
    -c commit.gpgSign=false commit -m "Demo baseline"
  git -C "$demo_dir/repo" worktree add -b demo "$demo_dir/worktree"
  git -C "$demo_dir/repo" worktree add -b clean-demo "$demo_dir/clean-worktree"
  printf 'Local note, absent from the commit\n' > "$demo_dir/worktree/notes.txt"
  initial_head="$(git -C "$demo_dir/repo" rev-parse HEAD)"
  "$sweep_bin" review --repo "$demo_dir/repo" --target refs/heads/main \
    --summary --run-dir "$demo_dir/reports/with-target"
  "$sweep_bin" review --repo "$demo_dir/repo" \
    --summary --run-dir "$demo_dir/reports/without-target"
  apply_status=0
  (cd "$demo_dir/repo"; "$sweep_bin" apply) > "$demo_dir/reports/apply.stdout" \
    2> "$demo_dir/reports/apply.stderr" || apply_status=$?
  test "$apply_status" -eq 2
  printf 'Local note, absent from the commit\n' | cmp - "$demo_dir/worktree/notes.txt"
  test "$(git -C "$demo_dir/repo" rev-parse HEAD)" = "$initial_head"
  test "$(git -C "$demo_dir/worktree" rev-parse HEAD)" = "$initial_head"
  test "$(git -C "$demo_dir/clean-worktree" rev-parse HEAD)" = "$initial_head"
  printf 'Note et HEAD inchangés ; apply a refusé avec le code 2.\n'
  printf 'Démo et rapports : %s\n' "$demo_dir"
)
```

Résultats attendus sur une collecte complète de cette fixture :

| Revue | `keep` | `cleanup_candidate` | `verify` |
| --- | --- | --- | --- |
| Avec cible | Principal et worktree contenant la note | Worktree lié propre, usage à confirmer | Aucun |
| Sans cible | Les mêmes deux worktrees | Aucun | Worktree lié propre, cible manquante |

La section `RESULTS` doit afficher `Keep: 2`, `Cleanup candidates: 1` et
`Needs verification: 0`. Sans cible, elle affiche respectivement 2, 0 et 1.
`COVERAGE` distingue les contrôles locaux complets, partiels et non collectés.
Une découverte complète ne signifie pas que l’analyse des fichiers ou de
l’historique est complète. `CHECKS STILL NEEDED` indique les conditions manquantes ;
`COLLECTION LIMITS` regroupe les limites de collecte observées.
Le chemin du rapport détaillé figure dans `REPORT`.
L’absence d’export forge reste inconnue ; le reçu peut être `partial` malgré le code `0`.

`review` collecte le résumé local courant sans manifeste détaillé. La note est
comptée comme non suivie dans `items[].local_summary`. Pour ses empreintes et
métadonnées, utilisez `scan --local` dans le parcours avancé. Le bloc vérifie la
note et les HEAD ; il ne démontre pas la conservation exhaustive du dépôt.

## 4. Lancer les tests automatiques

Depuis la racine du projet, avec les dépendances en cache :

```sh
(
  set -eu
  cargo fmt --all -- --check
  cargo clippy --all-targets --locked --offline -- -D warnings
  cargo test --locked --offline
  cargo build --locked --offline
  python3 -B -m unittest discover -s tests -p 'test_*.py' -v
  python3 -B scripts/sync-ai.py --check
)
```

Le build debug prépare le binaire utilisé par les tests Python. Le bloc s’arrête
au premier échec. Les tests possèdent leurs fixtures ; ils ne découvrent pas
votre machine. Lisez les `ignored` et `skipped` séparément des cas réussis.
Les oracles vérifient les artefacts et comparent aussi des états avant/après.
Ils ne valident ni suppression réelle, ni restauration, ni instantané machine.
Le [registre de finition](finish-ledger.md#triage-t01-t09) conserve les résultats
observés de la passe finale ; ce tutoriel donne une procédure et des attentes.

## Parcours avancé scan puis plan

Pour demander un manifeste détaillé ou séparer découverte et revue, renseignez
un clone et une cible confirmée, puis lancez :

```sh
repo_path="/absolute/path/to/your/clone"
target_ref="refs/heads/your-confirmed-target"
report_dir="$(mktemp -d /private/tmp/wt-sweep-manifest.XXXXXX)"
./target/release/wt-sweep scan --repo "$repo_path" --local \
  --out "$report_dir/inventory.json"
./target/release/wt-sweep plan --inventory "$report_dir/inventory.json" \
  --target "$target_ref" --summary --run-dir "$report_dir"
```

`plan` relit l’état courant ; le manifeste reste une observation de l’intervalle
du scan. Pour actualiser ses empreintes, effectuez un nouveau `scan --local` vers
une nouvelle sortie. `review` n’accepte ni `--local` ni `--inventory`.
Une archive `wt-sweep.review/1` est un ancien rapport, pas une entrée de `plan` ;
elle exige un scan neuf et ne fournit pas les contrôles courants C5.

Pour plusieurs racines, commencez par un inventaire sans cible universelle :

```sh
report_dir="$(mktemp -d /private/tmp/wt-sweep-roots.XXXXXX)"
./target/release/wt-sweep scan --root /absolute/path/to/projects \
  --root /absolute/path/to/worktrees --workers 4 \
  --out "$report_dir/inventory.json"
```

Créez la [carte de cibles](targets-and-filters.md#several-repositories) depuis les
identités observées et choisissez les refs voulues pour chaque famille.
Une carte vide ne choisit aucune cible. Si une identité physique change,
recréez la carte après un scan ; ne modifiez pas l’ancienne pour contourner un refus.

```sh
./target/release/wt-sweep plan --inventory "$report_dir/inventory.json" \
  --target-map "$report_dir/targets.json" --group-by repo \
  --summary --run-dir "$report_dir"
```

`review --root` accepte aussi une carte existante, les filtres, `--group-by`,
les options de contexte et l’export forge de `plan`. Les filtres agissent après
analyse. Pour collecter moins, choisissez `--repo` ou des racines plus étroites.

## Interruption et portée restante

La progression sur stderr indique le travail observé ; elle n’est pas un
checkpoint durable. Après une interruption ou une limite, lisez les lacunes,
la couverture restante et les omissions de publication. Cette couverture est
prudente et peut inclure des chemins déjà visités ; elle n’est pas une liste
exacte de reprise. Relancez une famille ou une racine ciblée vers une nouvelle
sortie. Les appels noyau bloqués peuvent retarder l’arrêt.

Un dossier incomplet sans reçu final n’établit pas un run sauvegardé. Si la
synchronisation finale échoue après apparition du reçu, la durabilité reste
inconnue. `review` refuse une source complète dépassant la limite de publication
et demande un périmètre plus étroit. Les omissions observées ne comptent pas les
dépôts jamais découverts. Consultez les [limites et codes de sortie](cli.md).

## Prompt à donner à un assistant

Claude Code et Codex peuvent utiliser directement le [skill du dépôt](assistant-skill.md).
Il fournit le parcours progressif et des prompts plus courts. Le prompt ci-dessous
reste utilisable par un assistant qui ne charge pas les skills du projet.

Remplacez les trois valeurs avant de copier ce prompt dans un assistant qui
dispose d’un terminal sur votre Mac. Worktree Sweep n’utilise aucun modèle.

```text
Examine les worktrees de ce dépôt avec Worktree Sweep, en lecture seule.

Sources de wt-sweep : <CHEMIN_ABSOLU_WT_SWEEP>
Clone à examiner : <CHEMIN_ABSOLU_DU_CLONE>
Cible d’intégration confirmée : <REFERENCE_COMPLETE>

Lis README.fr.md, docs/tutorial.fr.md et docs/cli.md. Vérifie le binaire, sa version, la révision source et la référence locale fournie. Lis review --help. Si une valeur manque ou ne peut pas être vérifiée, conserve ce blocage sans en deviner une autre.

Exécute review sur le clone et ses worktrees enregistrés avec cette cible explicite, --summary et --run-dir. Crée un nouveau dossier de rapports hors du clone, des worktrees et des administrations Git. Pour des empreintes détaillées, utilise le parcours documenté scan --local puis plan et précise pourquoi. Distingue le manifeste historique du résumé local courant.

Reste dans ce périmètre. Ne lance ni découverte globale, ni fetch, prune, gc, suppression de branche ou de worktree, ni apply. N’ajoute aucun réseau ou export forge implicite. Sans export fourni, conserve l’état de la forge inconnu.

Donne les commandes réellement exécutées, leurs codes de sortie, les chemins d’artefacts et leur reçu. Pour chaque worktree, présente chemin, branche, HEAD complet, verdict, raisons et conditions manquantes. Distingue faits observés, relations du graphe et preuves approfondies d’intégration. Décris fichiers locaux, histoire privée, stashes partagés et contexte de commits/changelog lorsqu’ils ont été observés, avec leurs limites.

Explique les compteurs complets, bornes minimales et inconnus. Pour une vue filtrée, conserve totaux et lacunes source. Après interruption ou limite, donne la raison, la portée restante et une commande de relance ciblée vers une nouvelle sortie. Une progression seule n’est pas un rapport sauvegardé.

Ne transforme ni cleanup_candidate en autorisation de suppression, ni un manifeste en sauvegarde. L’usage des candidats reste à confirmer. Les archives review/1 sont historiques et ne satisfont pas les contrôles courants C5. Termine par les vérifications humaines requises et les limites réellement observées. Ne déduis pas une couverture machine de cet essai par dépôt.
```

Si l’assistant ne peut pas exécuter les commandes, lancez le parcours vous-même
et transmettez un extrait expurgé. L’encodage hexadécimal des chemins reste
réversible ; il ne les anonymise pas.

## Si le résultat diffère

| Symptôme | Vérification |
| --- | --- |
| Dépendance absente hors ligne | Télécharger les dépendances avec `cargo fetch --locked`. |
| Garde macOS refusée | Conserver le diagnostic ; aucune option ne la contourne. |
| Candidat attendu encore `verify` | Lire les conditions C1-C5 manquantes, les refs et les lacunes de chaque domaine. |
| Sortie existante ou intérieure au périmètre | Créer une nouvelle base extérieure, avec des ancêtres ordinaires. |
| Arrêt ou publication refusée | Distinguer le rapport publié, les preuves partielles et la portée non examinée. |

Le [guide de dépannage](getting-started.md#troubleshooting) détaille les refus.
Pour signaler un échec, conservez commande, code, versions et révision testée,
avec un extrait expurgé selon les [consignes de contribution](../CONTRIBUTING.md#report-a-problem).

## Approfondir un worktree après une analyse partielle

Une limite de collecte ne signifie pas que le worktree est propre. Pour reprendre
les contrôles d’un chemin précis, lancer ce bloc depuis le checkout de `wt-sweep`
après compilation. Saisir le chemin exact affiché dans le rapport précédent.

```sh
(
  set -eu
  printf 'Chemin absolu du dépôt : '
  IFS= read -r repo_path
  printf 'Ref cible confirmée (refs/heads/... ou refs/remotes/...) : '
  IFS= read -r target_ref
  printf 'Chemin absolu du worktree à approfondir : '
  IFS= read -r worktree_path
  report_dir="$(mktemp -d /private/tmp/wt-sweep-report.XXXXXX)"
  ./target/release/wt-sweep review --repo "$repo_path" --target "$target_ref" \
    --worktree "$worktree_path" --path-prefix "$worktree_path" \
    --summary --run-dir "$report_dir"
  printf 'Rapports : %s\n' "$report_dir"
)
```

Ce passage réserve les lectures de fichiers et d’histoire au chemin choisi.
L’inventaire complet et les contrôles d’identité restent présents ; les autres
lignes portent leurs preuves manquantes. Les plafonds par worktree continuent
à s’appliquer. Le nouveau rapport ne complète pas rétroactivement l’ancien.
