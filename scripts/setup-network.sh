#!/usr/bin/env bash
# setup-network.sh: idempotent first-run setup for a z3 network.
#
# Copies per-network .example TOML templates into the live gitignored paths
# that docker-compose.yml mounts; generates a Zallet identity and shared TLS
# cert if missing.
#
# Usage:
#   ./scripts/setup-network.sh <mainnet|testnet|regtest>
#
# Safe to re-run: every step skips if its output already exists. The live
# TOMLs and identity file are local and gitignored.
#
# For regtest, this script only handles file setup; the operational steps
# (mining the activation block, generating the wallet mnemonic) live in
# scripts/regtest-init.sh, which delegates here first.

set -euo pipefail

NETWORK="${1:-}"
case "$NETWORK" in
    mainnet|testnet|regtest) ;;
    *)
        echo "Usage: $0 <mainnet|testnet|regtest>" >&2
        exit 1
        ;;
esac

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CONFIG_DIR="$REPO_ROOT/config/$NETWORK"

log() { printf '%s\n' "$*"; }

copy_template() {
    local file="$1"
    local example="$CONFIG_DIR/$file.example"
    local active="$CONFIG_DIR/$file"

    if [ -f "$active" ]; then
        log "==> $NETWORK/$file: present, leaving operator copy untouched."
        return
    fi

    if [ ! -f "$example" ]; then
        log "FAIL: missing template $example" >&2
        exit 1
    fi

    cp "$example" "$active"
    # Make the config world-readable so the pinned zallet container uid (1000)
    # can read it regardless of the operator's host uid/umask. These TOMLs are
    # not secret; the age key is handled separately in ensure_identity.
    chmod 644 "$active"
    log "==> $NETWORK/$file: created from .example template."
}

ensure_identity() {
    local identity="$CONFIG_DIR/zallet_identity.txt"

    if [ -f "$identity" ]; then
        log "==> $NETWORK/zallet_identity.txt: present."
        return
    fi

    if ! command -v rage-keygen >/dev/null 2>&1; then
        log "FAIL: rage-keygen not found." >&2
        log "      Install rage from https://github.com/str4d/rage/releases" >&2
        exit 1
    fi

    rage-keygen -o "$identity"
    chmod 600 "$identity"
    # Zallet runs as uid 1000 (distroless image, no runtime chown). Grant that
    # uid read access to the age key without widening it to other host users.
    if command -v setfacl >/dev/null 2>&1; then
        setfacl -m u:1000:r "$identity" \
            || log "WARN: setfacl failed on $identity; zallet (uid 1000) may not be able to read it."
    else
        log "WARN: setfacl not found. Grant uid 1000 read on $identity before starting zallet"
        log "      (install the 'acl' package, or 'chmod 644 $identity' to allow all local users)."
    fi
    log "==> $NETWORK/zallet_identity.txt: generated."
}

mkdir -p "$CONFIG_DIR"

copy_template zaino.toml
copy_template zallet.toml
# Regtest needs a Zebra TOML to activate NU5/NU6 at heights Zaino expects.
# Mainnet and testnet use Zebra's built-in network defaults.
if [ "$NETWORK" = "regtest" ]; then
    copy_template zebra.toml
fi
ensure_identity

log
log "Setup complete for $NETWORK."
log "Next: docker compose --env-file .env.$NETWORK up -d zebra"
