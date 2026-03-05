#!/usr/bin/env bash
# init.sh - Initialize the regtest wallet for the first time.
#
# Run this once before `docker compose up -d`.
# Safe to re-run: skips steps if already done.
#
# Requirements:
#   - Docker with the z3-zallet:local image built (cd ../../ && docker compose build zallet)
#   - No running z3_regtest_* containers

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
NETWORK=regtest_regtest_net

# Use sudo for docker if needed
DOCKER="docker"
if ! docker info > /dev/null 2>&1; then
    DOCKER="sudo -E docker"
fi
ZALLET_IMAGE=z3-zallet:local

cd "$SCRIPT_DIR"

echo "==> Starting Zebra in regtest mode..."
$DOCKER compose up -d zebra

echo "==> Waiting for Zebra RPC to be ready..."
until $DOCKER compose exec zebra curl -sf -X POST \
    -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","method":"getblockchaininfo","params":[],"id":1}' \
    http://127.0.0.1:18232 > /dev/null 2>&1; do
  echo "   Zebra not ready yet, retrying..."
  sleep 2
done
echo "   Zebra is ready."

echo "==> Mining 1 block (required for Orchard activation at height 1)..."
$DOCKER compose exec zebra curl -s -u zebra:zebra \
    -X POST -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","method":"generate","params":[1],"id":1}' \
    http://127.0.0.1:18232 | grep -q '"result"'
echo "   Block mined."

echo "==> Running init-wallet-encryption..."
$DOCKER compose run --rm zallet --datadir /var/lib/zallet init-wallet-encryption

echo "==> Running generate-mnemonic..."
$DOCKER compose run --rm zallet --datadir /var/lib/zallet generate-mnemonic

echo ""
echo "✅ Wallet initialized. Now run:"
echo "   sudo -E docker compose up -d"
echo "   # Router will be available at http://localhost:8181"
