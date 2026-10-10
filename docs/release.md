# Worktree Sweep v0.1.0-alpha.1

[Index documentaire](README.md) · [Changelog](../CHANGELOG.md) · [Feuille de route](roadmap.md)

Pré-release GitHub : [v0.1.0-alpha.1](https://github.com/FlorianBruniaux/wt-sweep/releases/tag/v0.1.0-alpha.1).
Le paquet vise macOS Apple Silicon (`aarch64-apple-darwin`) et conserve la licence FSL-1.1-ALv2. Il s’agit de Fair Source. La visibilité publique du repo ne change pas cette licence.

## Installer et vérifier

La [procédure de téléchargement](getting-started.md#download-the-apple-silicon-alpha) crée un dossier neuf, télécharge l’archive et `SHA256SUMS`, vérifie la somme puis exécute `--version` et `review --help`. Le binaire ne demande pas Rust. `/usr/bin/git` doit être disponible.

L’archive `wt-sweep-v0.1.0-alpha.1-aarch64-apple-darwin.tar.gz` contient le binaire, sa licence, les notices des dépendances et `MANIFEST.json`. Le manifeste identifie le commit source, le compilateur et les empreintes des fichiers. Gardez `SHA256SUMS` avec l’archive.

Utilisez le chemin absolu du binaire dans la [démo temporaire](getting-started.md#disposable-demo), à la place de `./target/release/wt-sweep`. Le résultat attendu est 2 `keep`, 1 `cleanup_candidate`, 0 `verify`. Les skills Claude/Codex restent dans le repo source.

Le binaire n’est pas notarisé par Apple. Le téléchargement par CLI et le lancement local ne qualifient pas Gatekeeper après téléchargement par navigateur. Aucun contournement de cette protection n’est demandé. Intel, Linux, Windows et une version minimale de macOS ne sont pas qualifiés. La recette précédente utilisait macOS 26.5.1 ; chaque nouvelle recette doit conserver son environnement exact.

## Périmètre de l’alpha

La CLI inventorie les familles Git choisies, mesure leur occupation disque et produit les verdicts `keep`, `verify` et `cleanup_candidate`. Les rapports texte, JSON et HTML conservent les lacunes de collecte. `--open` enregistre le rapport et demande son ouverture dans le navigateur par défaut ; sur macOS, le dossier par défaut est `~/Desktop/Worktree Sweep/`.

La version reste en lecture seule. `apply` refuse les demandes. Une taille allouée ne prédit pas les octets récupérables sur APFS ; un candidat ne prouve pas l’absence d’utilisation par un agent ou un terminal. Les grandes familles peuvent demander des reprises ciblées, même avec le budget `thorough`.

Les liens GitHub/GitLab proviennent des remotes locaux et des exports PR/MR associés. La CLI ne vérifie pas l’état actuel du serveur. L’évaluation du collecteur natif [#20](https://github.com/FlorianBruniaux/wt-sweep/issues/20) et les extensions [#14](https://github.com/FlorianBruniaux/wt-sweep/issues/14) sont hors de cette release.

## Reproduire le paquet

Les scripts de [packaging](../scripts/release/package.py) et de [recette](../scripts/release/accept-archive.py) sont versionnés. Ils ne publient rien et n’installent rien globalement. Lancez-les sur Apple Silicon depuis un clone neuf, propre, au commit retenu. Python 3.12+ est requis pour la recette de l’archive ; Rust 1.95.0 est utilisé pour compiler.

```sh
cargo fetch --locked
cargo build --release --locked --offline
release_check="$(mktemp -d /private/tmp/wt-sweep-package.XXXXXX)"
python3 -B scripts/release/package.py "$PWD" "$release_check/package"
python3 -B scripts/release/accept-archive.py "$PWD" "$release_check/package"
```

La recette vérifie les empreintes de l’archive et de ses fichiers, exécute l’aide et la démo documentaire, compare les verdicts attendus et vérifie les artefacts contre `run.json`. Ses fixtures restent dans des dossiers temporaires possédés ; aucun scan global n’est lancé.

## Contrôles de publication

Le mainteneur publie seulement après la CI produit réussie sur le commit exact à taguer, la compilation propre et la recette de l’archive. Il publie ensuite l’archive et `SHA256SUMS`, retélécharge les fichiers publics et rejoue leur recette avant de clore [#31](https://github.com/FlorianBruniaux/wt-sweep/issues/31).

La page de release et #31 portent les résultats de livraison, le commit et le lien CI. Le [registre](progress.md) conserve les observations historiques sans les transformer en validation du paquet courant. Aucun tap Homebrew n’est annoncé ; son évaluation reste facultative après cette publication.
