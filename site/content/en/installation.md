## Download the Apple Silicon alpha

Use [v0.1.0-alpha.2](https://github.com/FlorianBruniaux/wt-sweep/releases/tag/v0.1.0-alpha.2) on macOS Apple Silicon with `/usr/bin/git`. Rust is not required for the downloaded binary. The `Downloads` directory must exist; each installation uses a new folder.

```sh
(
  set -eu
  test "$(uname -s)" = Darwin
  test "$(uname -m)" = arm64
  install_dir="$(mktemp -d "$HOME/Downloads/wt-sweep-alpha.XXXXXX")"
  cd "$install_dir"
  release_url="https://github.com/FlorianBruniaux/wt-sweep/releases/download/v0.1.0-alpha.2"
  curl --fail --location --proto '=https' --tlsv1.2 -O "$release_url/SHA256SUMS"
  curl --fail --location --proto '=https' --tlsv1.2 -O "$release_url/wt-sweep-v0.1.0-alpha.2-aarch64-apple-darwin.tar.gz"
  shasum -a 256 -c SHA256SUMS
  tar -xzf wt-sweep-v0.1.0-alpha.2-aarch64-apple-darwin.tar.gz
  cd wt-sweep-v0.1.0-alpha.2-aarch64-apple-darwin
  ./wt-sweep --version
  ./wt-sweep review --help
  printf '\nBinary: %s/wt-sweep\n' "$PWD"
)
```

Use the printed binary path in place of `./target/release/wt-sweep` in the commands below. No global installation or permissions change is made. The archive includes licenses and `MANIFEST.json` with its source commit and file hashes. It is not Apple-notarized; browser-downloaded Gatekeeper behavior and older macOS versions remain unqualified. If macOS blocks execution, keep the protection and use the source-build instructions. Intel, Linux and Windows binaries are not provided.

## Requirements

Run on macOS with `/usr/bin/git`. Build using the Rust toolchain pinned in `rust-toolchain.toml` (1.95). The source version is `0.1.0-alpha.2`.

The public repository can be cloned over HTTPS. From an existing checkout, skip clone and `cd` below.

```sh
git clone https://github.com/FlorianBruniaux/wt-sweep.git
cd wt-sweep
cargo fetch --locked
cargo build --release --locked --offline
./target/release/wt-sweep --version
./target/release/wt-sweep review --help
```

`cargo fetch` downloads locked dependencies. Omit it if they are already cached. Rustup may also download the pinned toolchain. Python 3.11+ is for development tests and optional forge demos, not for running the CLI.

## Verify before collecting

Check `--version` and `review --help`, then [run the disposable demo](/docs/demo/). Never substitute a global scan for an installation test.

Before protected reads, wt-sweep establishes and verifies a native macOS guard against materializing dataless files. Unsupported platforms or guard failure cause refusal. Actual FileProvider placeholders and TCC denials remain unverified.

## Optional assistant

The checkout includes repository skills for Claude Code and Codex. [Open a session in this checkout](/docs/assistant/) and verify that your client recognizes the skill. Cloning does not install a global skill.
