## Prérequis

Utilisez macOS avec `/usr/bin/git`. Compilez avec le toolchain Rust fixé dans `rust-toolchain.toml` (1.95). La version Cargo actuelle est `0.1.0-alpha.1` ; cela ne prouve pas une release publiée.

L’accès GitHub est requis tant que le dépôt est privé. Depuis un checkout existant, omettez le clonage et le `cd` ci-dessous.

```sh
git clone https://github.com/FlorianBruniaux/wt-sweep.git
cd wt-sweep
cargo fetch --locked
cargo build --release --locked --offline
./target/release/wt-sweep --version
./target/release/wt-sweep review --help
```

`cargo fetch` télécharge les dépendances fixées. Omettez-le si elles sont déjà en cache. Rustup peut aussi télécharger le toolchain fixé. Python 3.11+ sert aux tests de développement et aux démos de forge facultatives, pas à l’exécution de la CLI.

## Vérifier avant de collecter

Contrôlez `--version` et `review --help`, puis [lancez la démo jetable](/fr/docs/demo/). Un scan global ne remplace pas un test d’installation.

Avant les lectures protégées, wt-sweep établit et vérifie une garde native macOS contre la matérialisation des fichiers dataless. Une plateforme non prise en charge ou une garde défaillante provoque un refus. Les vrais placeholders FileProvider et les refus TCC restent non éprouvés.

## Assistant facultatif

Le checkout contient les skills de dépôt pour Claude Code et Codex. [Ouvrez une session dans ce checkout](/fr/docs/assistant/) et vérifiez que le client reconnaît le skill. Cloner ne l’installe pas globalement.
