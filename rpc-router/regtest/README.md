# RPC Router — Regtest Environment

Self-contained Docker Compose setup for local end-to-end testing of the rpc-router against real Zebra and Zallet backends in regtest mode.

**Does not touch the production stack** in the repo root.

## Prerequisites

- Docker with the `z3-zallet:local` image built:
  ```bash
  cd ../../   # repo root
  sudo docker compose build zallet
  ```

## First-time setup

```bash
cd rpc-router/regtest
./init.sh
```

This will:
1. Start Zebra in regtest mode
2. Mine 1 block (activates Orchard at height 1)
3. Initialize the Zallet wallet (`init-wallet-encryption` + `generate-mnemonic`)

## Start the stack

```bash
docker compose up -d
```

Router is available at **http://localhost:8181**.

## Test routing

```bash
# Route to Zebra (full node)
curl -s -X POST -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"getblockchaininfo","params":[],"id":1}' \
  http://localhost:8181 | jq .

# Route to Zallet (wallet)
curl -s -X POST -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"getwalletinfo","params":[],"id":2}' \
  http://localhost:8181 | jq .

# Merged OpenRPC schema
curl -s -X POST -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"rpc.discover","params":[],"id":3}' \
  http://localhost:8181 | jq .info.title
```

## Stop and clean up

```bash
docker compose down -v   # -v removes the regtest volumes
```

## Notes

- Credentials: `zebra` / `zebra` (hardcoded for regtest only)
- Zallet uses regtest nuparams activating all upgrades at block 1
- The rpc-router Dockerfile is in `rpc-router/` (one level up)
