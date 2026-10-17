## Télécharger l’alpha Apple Silicon

Utilisez [v0.1.0-alpha.3](https://github.com/FlorianBruniaux/wt-sweep/releases/tag/v0.1.0-alpha.3) sur macOS Apple Silicon avec `/usr/bin/git`. Le binaire téléchargé ne demande pas Rust. Le dossier `Downloads` doit exister ; chaque installation crée un nouveau dossier.

```sh
(
  set -eu
  test "$(uname -s)" = Darwin
  test "$(uname -m)" = arm64
  install_dir="$(mktemp -d "$HOME/Downloads/wt-sweep-alpha.XXXXXX")"
  cd "$install_dir"
  release_url="https://github.com/FlorianBruniaux/wt-sweep/releases/download/v0.1.0-alpha.3"
  curl --fail --location --proto '=https' --tlsv1.2 -O "$release_url/SHA256SUMS"
  curl --fail --location --proto '=https' --tlsv1.2 -O "$release_url/wt-sweep-v0.1.0-alpha.3-aarch64-apple-darwin.tar.gz"
  shasum -a 256 -c SHA256SUMS
  tar -xzf wt-sweep-v0.1.0-alpha.3-aarch64-apple-darwin.tar.gz
  cd wt-sweep-v0.1.0-alpha.3-aarch64-apple-darwin
  ./wt-sweep --version
  ./wt-sweep review --help
  printf '\nBinary: %s/wt-sweep\n' "$PWD"
)
```

Utilisez le chemin du binaire affiché à la place de `./target/release/wt-sweep` dans les commandes. Aucun changement global de configuration ou de permissions. L’archive contient les licences et `MANIFEST.json`, avec le commit source et les empreintes des fichiers. Le binaire n’est pas notarisé par Apple ; Gatekeeper après téléchargement par navigateur et les anciennes versions de macOS restent non qualifiés. Si macOS bloque son exécution, conservez cette protection et compilez depuis les sources. Aucun binaire Intel, Linux ou Windows n’est proposé.

## Prérequis

Utilisez macOS avec `/usr/bin/git`. Compilez avec la version de Rust indiquée dans `rust-toolchain.toml` (1.95). La version des sources est `0.1.0-alpha.3`.

Clonez le repo public en HTTPS. Si vous avez déjà un checkout, omettez le clonage et le `cd` ci-dessous.

```sh
git clone https://github.com/FlorianBruniaux/wt-sweep.git
cd wt-sweep
cargo fetch --locked
cargo build --release --locked --offline
./target/release/wt-sweep --version
./target/release/wt-sweep review --help
```

`cargo fetch` télécharge les dépendances du lockfile. Omettez-le si elles sont déjà en cache. Rustup peut aussi télécharger la version de Rust demandée. Python 3.11+ sert aux tests de développement et aux démos facultatives d’import GitHub ou GitLab, pas à l’exécution de la CLI.

## Vérifier avant de collecter

Vérifiez `--version` et `review --help`, puis [lancez la démo sur un repo de test](/fr/docs/demo/). Un scan global ne remplace pas un test d’installation.

Avant les lectures protégées, wt-sweep active et vérifie un contrôle macOS contre la matérialisation des fichiers dataless, dont le contenu n’est pas présent localement. Si la plateforme ne le prend pas en charge ou si ce contrôle échoue, wt-sweep refuse de poursuivre. Le comportement sur de vrais placeholders FileProvider et refus TCC reste non vérifié.

## Assistant facultatif

Le checkout contient les skills pour Claude Code et Codex, limités à ce repo. [Ouvrez une session dans ce checkout](/fr/docs/assistant/) et vérifiez que le client reconnaît le skill. Le clonage ne les installe pas globalement.
