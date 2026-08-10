#!/usr/bin/env bash
# Re-vendors Google's OKF sample bundles into tests/fixtures/upstream/.
#
# These bundles are the anti-false-positive net: `okf validate` must report
# zero errors on every one of them. Bump UPSTREAM_COMMIT deliberately, then
# re-run the test suite -- a new upstream commit changing the samples is
# exactly the signal we want to catch.
set -euo pipefail

UPSTREAM_REPO="GoogleCloudPlatform/knowledge-catalog"
UPSTREAM_COMMIT="374e0bc4c644310ff56cdf9c0fe81eccdec862b0"

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dest="${repo_root}/tests/fixtures/upstream"
workdir="$(mktemp -d)"
trap 'rm -rf "${workdir}"' EXIT

echo "Fetching ${UPSTREAM_REPO}@${UPSTREAM_COMMIT}..."
curl -fsSL "https://codeload.github.com/${UPSTREAM_REPO}/tar.gz/${UPSTREAM_COMMIT}" \
    | tar -xz -C "${workdir}"

src="${workdir}/knowledge-catalog-${UPSTREAM_COMMIT}/okf/bundles"
[ -d "${src}" ] || { echo "error: ${src} not found in upstream tarball" >&2; exit 1; }

rm -rf "${dest}"
mkdir -p "${dest}"
cp -R "${src}"/* "${dest}/"

cat > "${dest}/NOTICE" <<EOF
The bundles in this directory are vendored, unmodified, from:

    https://github.com/${UPSTREAM_REPO}
    commit ${UPSTREAM_COMMIT}
    path   okf/bundles/

Copyright Google LLC, licensed under the Apache License 2.0. See the LICENSE
file at the root of this repository for the full license text.

Regenerate with scripts/refresh-fixtures.sh -- do not edit these files by hand.
EOF

echo "Vendored $(find "${dest}" -name '*.md' | wc -l | tr -d ' ') markdown files into ${dest}"
