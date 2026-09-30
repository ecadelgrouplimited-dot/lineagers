#!/bin/sh
# Installs the Lineage CLI (`lineage`) and `guard-server`.
#
#   curl -fsSL https://lineagrs.tech/install.sh | sh
#
# Options (environment variables):
#   LINEAGE_VERSION       version to install (default: latest)
#   LINEAGE_INSTALL_DIR   where to put the binaries (default: ~/.local/bin)
#
# The download is checked against the published SHA-256 checksums before anything
# is installed.
set -eu

BASE_URL="${LINEAGE_BASE_URL:-https://lineagrs.tech/downloads}"
INSTALL_DIR="${LINEAGE_INSTALL_DIR:-$HOME/.local/bin}"

say() { printf 'lineage-install: %s\n' "$*"; }
fail() { printf 'lineage-install: error: %s\n' "$*" >&2; exit 1; }

fetch() {
  if command -v curl >/dev/null 2>&1; then curl -fsSL "$1" -o "$2"
  elif command -v wget >/dev/null 2>&1; then wget -qO "$2" "$1"
  else fail "need curl or wget"
  fi
}

os=$(uname -s)
arch=$(uname -m)
if [ "$os" != "Linux" ] || [ "$arch" != "x86_64" ]; then
  fail "prebuilt binaries are available for Linux x86_64 only (this is $os $arch).
  Install from source instead:  cargo install lineage-rs
  and see https://docs.lineagrs.tech/getting-started/installation.html"
fi

if command -v sha256sum >/dev/null 2>&1; then sha256() { sha256sum "$1" | cut -d' ' -f1; }
elif command -v shasum >/dev/null 2>&1; then sha256() { shasum -a 256 "$1" | cut -d' ' -f1; }
else fail "need sha256sum or shasum to verify the download"
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

version="${LINEAGE_VERSION:-}"
if [ -z "$version" ]; then
  fetch "$BASE_URL/latest.txt" "$tmp/latest.txt"
  version=$(tr -d ' \n\r' < "$tmp/latest.txt")
fi
name="lineage-$version-x86_64-linux"

say "downloading lineage $version"
fetch "$BASE_URL/$version/$name.tar.gz" "$tmp/$name.tar.gz"
fetch "$BASE_URL/$version/SHA256SUMS" "$tmp/SHA256SUMS"

expected=$(grep " $name.tar.gz\$" "$tmp/SHA256SUMS" | cut -d' ' -f1)
[ -n "$expected" ] || fail "no checksum for $name.tar.gz"
actual=$(sha256 "$tmp/$name.tar.gz")
[ "$expected" = "$actual" ] || fail "checksum mismatch for $name.tar.gz (expected $expected, got $actual)"
say "checksum verified"

tar -C "$tmp" -xzf "$tmp/$name.tar.gz"
mkdir -p "$INSTALL_DIR"
for bin in lineage guard-server; do
  install -m 0755 "$tmp/$name/$bin" "$INSTALL_DIR/$bin"
done
say "installed lineage and guard-server to $INSTALL_DIR"

case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *) say "add $INSTALL_DIR to your PATH:  export PATH=\"$INSTALL_DIR:\$PATH\"" ;;
esac
say "next: lineage --help   |   guard-server   |   https://docs.lineagrs.tech"
