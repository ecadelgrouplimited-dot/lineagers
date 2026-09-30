#!/usr/bin/env bash
# Builds the downloadable release into dist/<version>/:
#   lineage-<version>-x86_64-linux.tar.gz   static `lineage` CLI and `guard-server`
#   lineage-<version>-src.tar.gz             source of the tagged tree (git archive)
#   lineage_guard.py                         dependency-free Python client
#   SHA256SUMS
# and dist/latest.txt containing the version.
#
# Usage: scripts/package-release.sh            (run from anywhere in the repo)
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
target=x86_64-unknown-linux-musl
out="dist/$version"
stage="dist/.stage/lineage-$version-x86_64-linux"

rustup target list --installed | grep -qx "$target" || rustup target add "$target"

echo "building lineage $version for $target"
cargo build --release --locked --target "$target" --bin lineage --no-default-features --features cli
cargo build --release --locked --target "$target" --manifest-path apps/guard-server/Cargo.toml

rm -rf "$out" "dist/.stage"
mkdir -p "$out" "$stage"
cp "target/$target/release/lineage" "apps/guard-server/target/$target/release/guard-server" "$stage/"
cp README.md LICENSE CHANGELOG.md "$stage/"
cp apps/guard-server/clients/python/lineage_guard.py "$stage/"

tar -C dist/.stage -czf "$out/lineage-$version-x86_64-linux.tar.gz" "lineage-$version-x86_64-linux"
git archive --format=tar.gz --prefix="lineage-$version/" -o "$out/lineage-$version-src.tar.gz" HEAD
cp apps/guard-server/clients/python/lineage_guard.py "$out/"
(cd "$out" && sha256sum -- *.tar.gz lineage_guard.py > SHA256SUMS)
echo "$version" > dist/latest.txt
rm -rf "dist/.stage"

echo
ls -l "$out"
cat "$out/SHA256SUMS"
