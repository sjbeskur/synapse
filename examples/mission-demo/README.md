# Mission Demo

This example shows Synapse checking several cFS app message definitions as one mission-visible packet set.

The useful part is not generated code. The useful part is validation across app boundaries:

- Telemetry MIDs must be unique across the checked mission roots.
- Command MID/CC pairs must be unique across the checked mission roots.
- A shared command MID is allowed when command codes differ.

## Valid Mission Check

```bash
cargo run -p cfs-synapse -- check \
  examples/mission-demo/syn/nav_app.syn \
  examples/mission-demo/syn/camera_app.syn \
  examples/mission-demo/syn/payload_app.syn
```

That command validates three app roots plus their shared `mission_ids.syn` import.

## Intentional Telemetry Conflict

```bash
cargo run -p cfs-synapse -- check \
  examples/mission-demo/syn/nav_app.syn \
  examples/mission-demo/conflicts/duplicate_tlm_mid.syn
```

Expected result:

```text
duplicate telemetry MID `0x0801`
```

## Intentional Command Conflict

```bash
cargo run -p cfs-synapse -- check \
  examples/mission-demo/syn/camera_app.syn \
  examples/mission-demo/conflicts/duplicate_cmd_cc.syn
```

Expected result:

```text
duplicate command MID/CC pair `0x1881`/`1`
```

## Generate HTML Documentation

```bash
cargo run -p cfs-synapse -- doc -o /tmp/synapse-mission-docs \
  examples/mission-demo/syn/nav_app.syn \
  examples/mission-demo/syn/camera_app.syn \
  examples/mission-demo/syn/payload_app.syn
```

That writes `/tmp/synapse-mission-docs/index.html` with packet IDs, command codes, fields, types, and doc comments.

## Export A Packet Registry

```bash
cargo run -p cfs-synapse -- registry --format json -o /tmp/synapse-mission-registry.json \
  examples/mission-demo/syn/nav_app.syn \
  examples/mission-demo/syn/camera_app.syn \
  examples/mission-demo/syn/payload_app.syn

cargo run -p cfs-synapse -- registry --format csv -o /tmp/synapse-mission-registry.csv \
  examples/mission-demo/syn/nav_app.syn \
  examples/mission-demo/syn/camera_app.syn \
  examples/mission-demo/syn/payload_app.syn
```

Those commands write validated packet inventories for downstream databases, reports, or ICD tooling.
