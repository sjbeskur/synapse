# Mission Demo

This example shows Synapse checking several reusable cFS app message
definitions against mission-owned topic assignments.

The `.syn` files define logical command groups and telemetry topics without
deployment MIDs. `mission.toml` assigns cFE topic IDs:

- Command and telemetry topic IDs are unique within their respective spaces.
- Every logical topic must have an assignment.
- Function codes are unique within a command topic.
- Stale or mistyped manifest entries fail validation.

## Valid Mission Check

```bash
cargo run -p cfs-synapse -- check \
  --manifest examples/mission-demo/mission.toml \
  examples/mission-demo/syn/nav_app.syn \
  examples/mission-demo/syn/camera_app.syn \
  examples/mission-demo/syn/payload_app.syn
```

That command validates three app roots and every assignment in the mission
manifest.

## Generate The cFE Routing Header

```bash
cargo run -p cfs-synapse -- routes \
  --manifest examples/mission-demo/mission.toml \
  -o /tmp/synapse-mission-topics.h \
  examples/mission-demo/syn/nav_app.syn \
  examples/mission-demo/syn/camera_app.syn \
  examples/mission-demo/syn/payload_app.syn
```

The generated header maps each topic ID through
`CFE_PLATFORM_CMD_TOPICID_TO_MIDV` or
`CFE_PLATFORM_TLM_TOPICID_TO_MIDV`.

## Intentional Telemetry Topic Conflict

```bash
cargo run -p cfs-synapse -- check \
  examples/mission-demo/syn/nav_app.syn \
  examples/mission-demo/conflicts/duplicate_tlm_mid.syn
```

Expected result:

```text
duplicate telemetry topic `nav_app::NavState`
```

## Intentional Function-Code Conflict

```bash
cargo run -p cfs-synapse -- check \
  examples/mission-demo/syn/camera_app.syn \
  examples/mission-demo/conflicts/duplicate_cmd_cc.syn
```

Expected result:

```text
duplicate function code `1` for command topic `camera_app::CameraCommands`
```

## Generate HTML Documentation

```bash
cargo run -p cfs-synapse -- doc -o /tmp/synapse-mission-docs \
  examples/mission-demo/syn/nav_app.syn \
  examples/mission-demo/syn/camera_app.syn \
  examples/mission-demo/syn/payload_app.syn
```

That writes `/tmp/synapse-mission-docs/index.html` with logical topics, command
codes, fields, types, and doc comments.

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
