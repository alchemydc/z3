# Z3 Observability Stack

Metrics, alerting, and dashboards for the Z3 stack (Zebra, Zaino, Zallet).

## Quick Start

```bash
# 1. Enable Zebra metrics in .env (uncomment this line):
ZEBRA_METRICS__ENDPOINT_ADDR=0.0.0.0:9999

# 2. Start the full stack with monitoring
docker compose --profile monitoring up -d

# 3. View logs
docker compose logs -f zebra
```

> **Note**: OpenTelemetry tracing (Jaeger) is not yet available in Zebra 4.0.0.
> Jaeger is included for use with future Zebra releases that support tracing.

## Components

| Component | Port | URL | Purpose |
|-----------|------|-----|---------|
| **Zebra** | 9999 | - | Zcash node with metrics and tracing |
| **Prometheus** | 9094 | <http://localhost:9094> | Metrics collection and storage |
| **Grafana** | 3000 | <http://localhost:3000> | Dashboards and visualization |
| **Jaeger** | 16686 | <http://localhost:16686> | Distributed tracing UI |
| **AlertManager** | 9093 | <http://localhost:9093> | Alert routing |

Default Grafana credentials: `admin` / `admin` (you'll be prompted to change on first login)

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Zebra Node                                  │
│  ┌─────────────────┐              ┌─────────────────────────────┐   │
│  │ Metrics         │              │ Tracing (OpenTelemetry)     │   │
│  │ :9999/metrics   │              │ OTLP HTTP → Jaeger          │   │
│  └────────┬────────┘              └──────────────┬──────────────┘   │
└───────────│──────────────────────────────────────│──────────────────┘
            │                                      │
            ▼                                      ▼
┌───────────────────┐                  ┌───────────────────────────┐
│   Prometheus      │                  │        Jaeger             │
│   :9094           │                  │   :16686 (UI)             │
│                   │◄─────────────────│   :8889 (spanmetrics)     │
│   Scrapes metrics │  Span metrics    │   :4318 (OTLP HTTP)       │
└─────────┬─────────┘                  └───────────────────────────┘
          │                                        │
          ▼                                        │
┌───────────────────┐                              │
│     Grafana       │◄─────────────────────────────┘
│     :3000         │      Trace queries
│                   │
│  Dashboards for   │
│  metrics + traces │
└─────────┬─────────┘
          │
          ▼
┌───────────────────┐
│   AlertManager    │
│   :9093           │
│                   │
│  Routes alerts    │
└───────────────────┘
```

## What Each Component Provides

### Metrics (Prometheus + Grafana)

Quantitative data about Zebra's behavior over time:

- **Network health**: Peer connections, bandwidth, message rates
- **Sync progress**: Block height, checkpoint verification, chain tip
- **Performance**: Block/transaction verification times
- **Resources**: Memory, connections, queue depths

See [grafana/README.md](grafana/README.md) for dashboard details.

### Tracing (Jaeger)

> **Note**: OpenTelemetry tracing is not yet available in Zebra 4.0.0.
> This feature will be supported in a future Zebra release.

Once available, Jaeger will provide:

- **Distributed traces**: Follow a request through all components
- **Latency breakdown**: See where time is spent in each operation
- **Error analysis**: Identify failure points and error propagation
- **Service Performance Monitoring (SPM)**: RED metrics for RPC endpoints

See [jaeger/README.md](jaeger/README.md) for tracing details.

### Alerts (AlertManager)

Automated notifications for operational issues:

- Critical: Negative value pools (ZIP-209 violation)
- Warning: High RPC latency, sync stalls, peer connection issues

Configure alert destinations in [alertmanager/alertmanager.yml](alertmanager/alertmanager.yml).

## Configuration

### Environment Variables

Add this to your `.env` file to enable Zebra metrics:

| Variable | Default | Description |
|----------|---------|-------------|
| `ZEBRA_METRICS__ENDPOINT_ADDR` | - | Prometheus metrics endpoint (e.g., `0.0.0.0:9999`) |

### Port Customization

Override default ports in `.env`:

```bash
GRAFANA_PORT=3000
PROMETHEUS_PORT=9094
JAEGER_UI_PORT=16686
ALERTMANAGER_PORT=9093
```

## Common Tasks

### View Zebra's current metrics

```bash
curl -s http://localhost:9999/metrics | grep zcash
```

### Query Prometheus directly

```bash
# Current block height
curl -s 'http://localhost:9094/api/v1/query?query=zcash_state_tip_height'
```

## Troubleshooting

### No metrics in Grafana

1. Verify `ZEBRA_METRICS__ENDPOINT_ADDR=0.0.0.0:9999` is set in `.env`
2. Restart Zebra: `docker compose restart zebra`
3. Check Zebra is exposing metrics: `docker compose exec zebra wget -qO- http://localhost:9999/metrics | head`
4. Check Prometheus targets: <http://localhost:9094/targets>

## Running Without Monitoring

To run the Z3 stack without monitoring:

```bash
docker compose up -d  # Only starts zebra, zaino, zallet
```

To add monitoring later:

```bash
docker compose --profile monitoring up -d
```
