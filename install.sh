#!/bin/sh
set -eu

repo="freeo/jtv"
install_dir="${JTV_INSTALL_DIR:-$HOME/.local/bin}"
version="${JTV_VERSION:-latest}"
command="${1:-install}"

case "$(uname -s)" in
  Linux)
    case "$(uname -m)" in
      x86_64|amd64) asset="jtv-linux-amd64" ;;
      *) echo "jtv Linux releases currently support x86_64 only" >&2; exit 1 ;;
    esac
    ;;
  Darwin) asset="jtv-macos-$(uname -m)" ;;
  *) echo "jtv releases currently support Linux and macOS only" >&2; exit 1 ;;
esac

case "$version" in
  latest) release_url="https://github.com/$repo/releases/latest/download" ;;
  v[0-9]*) release_url="https://github.com/$repo/releases/download/$version" ;;
  [0-9]*) release_url="https://github.com/$repo/releases/download/v$version" ;;
  *) echo "JTV_VERSION must be latest or a version such as 0.4.0" >&2; exit 2 ;;
esac

case "$command" in
  uninstall)
    if [ -e "$install_dir/jtv" ]; then
      rm "$install_dir/jtv"
      echo "removed $install_dir/jtv"
    else
      echo "jtv is not installed at $install_dir/jtv"
    fi
    exit 0
    ;;
  install) ;;
  *) echo "usage: install.sh [install|uninstall]" >&2; exit 2 ;;
esac

command -v curl >/dev/null || { echo "curl is required" >&2; exit 1; }

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT HUP INT TERM

curl -fsSL "$release_url/$asset" -o "$tmpdir/$asset"
curl -fsSL "$release_url/$asset.sha256" -o "$tmpdir/$asset.sha256"

if command -v sha256sum >/dev/null; then
  (cd "$tmpdir" && sha256sum -c "$asset.sha256")
elif command -v shasum >/dev/null; then
  (cd "$tmpdir" && shasum -a 256 -c "$asset.sha256")
else
  echo "sha256sum or shasum is required" >&2
  exit 1
fi

install -d "$install_dir"
install -m 0755 "$tmpdir/$asset" "$install_dir/jtv"
echo "installed jtv to $install_dir/jtv"

case ":$PATH:" in
  *":$install_dir:"*) ;;
  *) echo "add $install_dir to PATH to run jtv" ;;
esac
