#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WWWROOT="${SCRIPT_DIR}/../wwwroot"
DX_OUT="${SCRIPT_DIR}/target/dx/my-no-sql-ui/release/web/public"

cd "${SCRIPT_DIR}"

# dx never clears its output folder, so the bundles of every previous build pile
# up there under old hashes - and would all be copied into wwwroot below.
echo ">> cleaning ${DX_OUT}"
rm -rf "${DX_OUT}"

echo ">> dx build --release --web"
dx build --release --web

if [ ! -d "${DX_OUT}" ]; then
    echo "ERROR: build output not found at ${DX_OUT}"
    exit 1
fi

echo ">> cleaning ${WWWROOT}"
rm -rf "${WWWROOT}"
mkdir -p "${WWWROOT}"

echo ">> copying ${DX_OUT}/. -> ${WWWROOT}/"
cp -R "${DX_OUT}/." "${WWWROOT}/"

echo ">> done."
