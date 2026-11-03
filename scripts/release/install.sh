#!/bin/bash
# Install an already downloaded, checksum-verified release. No network or sudo.
set -eu
fail() { printf 'Install failed: %s\n' "$*" >&2; exit 1; }
package_dir="$(cd -- "$(dirname -- "$0")" && pwd -P)"
prefix="${HOME:?HOME must be set}/.local"
replace=false
while [ "$#" -gt 0 ]; do
  case "$1" in
    --prefix|--package)
      [ "$#" -ge 2 ] && [ -n "$2" ] || fail "$1 needs a directory"
      case "$1" in --prefix) prefix="$2" ;; --package) package_dir="$2" ;; esac
      shift 2 ;;
    --replace) replace=true; shift ;;
    --help)
      printf '%s\n' 'Usage: bash install.sh [--prefix DIR] [--replace]' \
        'Default prefix: ~/.local. Installs bin/wt-sweep and lib/wt-sweep/VERSION.' \
        '--replace switches an existing launcher; previous version files are retained.' \
        '--package DIR selects an extracted release (default: this script directory).' \
        'Verify the downloaded archive against its published SHA256SUMS first.' \
        'No sudo, shell configuration changes, network requests or worktree scans.'
      exit 0 ;;
    *) fail "unknown option: $1" ;;
  esac
done
case "$prefix" in /*) ;; *) fail '--prefix must be absolute' ;; esac
package_dir="$(cd -- "$package_dir" && pwd -P)"
# Resolve existing parent aliases before checking for a recursive copy.
case "$prefix/" in */../*|*/./*) fail '--prefix must not contain . or .. components' ;; esac
parent="$prefix"
suffix=''
while [ ! -d "$parent" ]; do
  suffix="/$(basename -- "$parent")$suffix"
  parent="$(dirname -- "$parent")"
done
prefix="$(cd -- "$parent" && pwd -P)$suffix"
case "$prefix/" in "$package_dir/"*) fail 'installation prefix must be outside the package' ;; esac
[ -f "$package_dir/FILES.sha256" ] || fail 'release checksums missing'
(cd -- "$package_dir" && /usr/bin/shasum -a 256 -c FILES.sha256 >/dev/null) || fail 'release checksum mismatch'
version="$(cat "$package_dir/VERSION")"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[A-Za-z0-9.]+)?$ ]] || fail 'invalid release version'
[ -x "$package_dir/wt-sweep" ] || fail 'release binary is not executable'
[ "$("$package_dir/wt-sweep" --version)" = "wt-sweep $version" ] || fail 'binary version mismatch'
destination="$prefix/lib/wt-sweep/$version"
launcher="$prefix/bin/wt-sweep"
[ ! -e "$destination" ] && [ ! -L "$destination" ] || fail "version already installed: $destination (use a new prefix to reinstall)"
[ ! -d "$launcher" ] || fail "launcher is a directory: $launcher"
if { [ -e "$launcher" ] || [ -L "$launcher" ]; } && [ "$replace" = false ]; then
  fail "command already exists: $launcher; use --replace to switch it"
fi
mkdir -p -- "$prefix/lib/wt-sweep" "$prefix/bin"
# mkdir is the no-clobber reservation, including against another installer.
mkdir -- "$destination" || fail "cannot reserve $destination"
committed=false
link_dir=''
cleanup() {
  if [ -n "$link_dir" ]; then rm -f -- "$link_dir/wt-sweep"; rmdir -- "$link_dir"; fi
  if [ "$committed" = false ]; then rm -rf -- "$destination"; fi
}
trap cleanup EXIT
cp -R -- "$package_dir/." "$destination/"
(cd -- "$destination" && /usr/bin/shasum -a 256 -c FILES.sha256 >/dev/null) || fail 'installed checksum mismatch'
if [ "$replace" = true ]; then
  link_dir="$(mktemp -d "$prefix/bin/.wt-sweep-install.XXXXXX")"
  ln -s -- "$destination/wt-sweep" "$link_dir/wt-sweep"
  # macOS -h replaces a symlink itself instead of following a directory link.
  /bin/mv -fh -- "$link_dir/wt-sweep" "$launcher"
else
  ln -s -- "$destination/wt-sweep" "$launcher"
fi
committed=true
printf '\nInstalled: %s\nVersion: %s\n' "$launcher" "$version"
printf 'Use the absolute path above, or add this directory to PATH: %s/bin\n' "$prefix"
printf 'Next: "%s" plan --help\n' "$launcher"
