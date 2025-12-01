#!/bin/bash
HOST="z3.localhost"
URL="http://localhost"

# Zebra RPC requires cookie auth so we pass it here
# Zallet and Zaino RPCs do not require auth
# but Zaino's RPC seems to be exposed only via TLS which we still need to workaround
ZEBRA_COOKIE="__cookie__:YOUR_COOKIE"

call_rpc() {
    method=$1
    echo "---------------------------------------------------"
    echo "Calling $method..."
    curl -s -X POST -H "Host: $HOST" -H "Content-Type: application/json" -u "$ZEBRA_COOKIE" \
        -d "{\"jsonrpc\": \"2.0\", \"method\": \"$method\", \"params\": [], \"id\": 1}" \
        "$URL"
    echo -e "\n"
}

# Zebra method
call_rpc "getblock"

# Zallet method
call_rpc "z_sendmany"

# Zaino method
call_rpc "getaddressbalance"
