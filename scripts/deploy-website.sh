#!/usr/bin/env bash
# Builds the docs and publishes the website, docs, and release downloads to the web host.
#
#   scripts/deploy-website.sh                 # site + docs + dist/<version> downloads
#   LINEAGE_DEPLOY_HOST=user@host scripts/deploy-website.sh
#
# Expects the nginx config in deploy/nginx/lineagrs.tech.conf to be installed on the host,
# and dist/ to be built by scripts/package-release.sh. Static files only: nginx needs no reload.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"
host="${LINEAGE_DEPLOY_HOST:-root@72.62.185.212}"
root=/var/www/lineagrs
version=$(cat dist/latest.txt)
[ -f "dist/$version/SHA256SUMS" ] || { echo "dist/$version is missing; run scripts/package-release.sh" >&2; exit 1; }

"${MDBOOK:-mdbook}" build docs/book

rsync -az --delete --exclude 'downloads/[0-9]*' --exclude downloads/latest.txt website/ "$host:$root/site/"
rsync -az "dist/$version" "$host:$root/site/downloads/"
rsync -az --delete docs/book/book/ "$host:$root/docs/"
# latest.txt last, so the install script never points at a version that isn't uploaded yet.
rsync -az dist/latest.txt "$host:$root/site/downloads/latest.txt"
ssh "$host" "chown -R root:root $root && find $root -type d -exec chmod 755 {} + && find $root -type f -exec chmod 644 {} +"

for url in https://lineagrs.tech/ https://lineagrs.tech/downloads/latest.txt "https://lineagrs.tech/downloads/$version/SHA256SUMS" https://docs.lineagrs.tech/; do
  printf '%s  %s\n' "$(curl -s -o /dev/null -w '%{http_code}' "$url")" "$url"
done
