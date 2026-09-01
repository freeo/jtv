#!/bin/sh
set -eu

repo="freeo/jtv"
asset="jtv-linux-amd64"
install_dir="${JTV_INSTALL_DIR:-/usr/local/bin}"
version="${JTV_VERSION:-latest}"

case "$(uname -s)" in
  Linux) ;;
  *) echo "jtv releases currently support Linux only" >&2; exit 1 ;;
esac

case "$(uname -m)" in
  x86_64|amd64) ;;
  *) echo "jtv releases currently support x86_64 only" >&2; exit 1 ;;
esac

case "$version" in
  latest) release_url="https://github.com/$repo/releases/latest/download" ;;
  v[0-9]*) release_url="https://github.com/$repo/releases/download/$version" ;;
  [0-9]*) release_url="https://github.com/$repo/releases/download/v$version" ;;
  *) echo "JTV_VERSION must be latest or a version such as 0.4.0" >&2; exit 2 ;;
esac

command -v curl >/dev/null || { echo "curl is required" >&2; exit 1; }
command -v sha256sum >/dev/null || { echo "sha256sum is required" >&2; exit 1; }

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT HUP INT TERM

curl -fsSL "$release_url/$asset" -o "$tmpdir/$asset"
curl -fsSL "$release_url/$asset.sha256" -o "$tmpdir/$asset.sha256"
(cd "$tmpdir" && sha256sum -c "$asset.sha256")

install -d "$install_dir"
install -m 0755 "$tmpdir/$asset" "$install_dir/jtv"
echo "installed jtv to $install_dir/jtv"
