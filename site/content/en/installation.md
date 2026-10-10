## Requirements

Run on macOS with `/usr/bin/git`. Build using the Rust toolchain pinned in `rust-toolchain.toml` (1.95). The current Cargo version is `0.1.0-alpha.1`; that does not establish a published release.

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
