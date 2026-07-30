# Packet Registry Output

Synapse can export a validated packet registry from one or more `.syn` roots:

```bash
synapse registry mission/nav_app.syn mission/camera_app.syn
synapse registry --format csv -o packets.csv mission/nav_app.syn mission/camera_app.syn
```

The registry is an export artifact. It is not the source of truth and it is not a replacement for `.syn` files. The `.syn` files remain the reviewed, versioned message contracts; registry output is for tools that need a flat packet inventory.

## What Is Exported

The current registry is packet-level. It includes `command` and `telemetry`
declarations with logical topics and command codes. It does not yet export
every struct, table, enum, constant, or field-level schema.

Before writing registry output, Synapse loads the same import graph used for generation and validates cFS packet facts. That means registry export fails on problems such as:

- Missing `@cc(...)` on commands.
- `@cc(...)` used on telemetry, structs, or tables.
- Duplicate function codes within one logical command topic.
- Unresolved or non-integer command-code attributes.
- Schema-level `@mid(...)` attributes.
- Duplicate logical telemetry topics across the collected roots.
- Duplicate command topic/function-code pairs across the collected roots.

When multiple roots are passed to `synapse registry`, the output contains packet declarations from the validated roots and their loaded import closures. The command applies the same mission-wide duplicate checks as `synapse check` before writing output.

## JSON Format

JSON output is the default:

```bash
synapse registry mission/camera_app.syn
```

Example:

```json
{
  "packets": [
    {
      "namespace": "camera_app",
      "name": "SetExposure",
      "qualified_name": "camera_app::SetExposure",
      "kind": "command",
      "source": "mission/camera_app.syn",
      "topic": "CameraCommands",
      "cc": 2
    },
    {
      "namespace": "camera_app",
      "name": "CameraStatus",
      "qualified_name": "camera_app::CameraStatus",
      "kind": "telemetry",
      "source": "mission/camera_app.syn",
      "topic": "CameraStatus",
      "cc": null
    }
  ]
}
```

## CSV Format

CSV output uses the same fields as JSON:

```bash
synapse registry --format csv -o packets.csv mission/camera_app.syn
```

Header:

```csv
namespace,name,qualified_name,kind,source,topic,cc
```

Example rows:

```csv
"camera_app","SetExposure","camera_app::SetExposure","command","mission/camera_app.syn","CameraCommands","2"
"camera_app","CameraStatus","camera_app::CameraStatus","telemetry","mission/camera_app.syn","CameraStatus",""
```

All CSV fields are quoted. Telemetry packets use an empty `cc` field.

## Field Reference

| Field | Meaning |
| --- | --- |
| `namespace` | Namespace declared by the source file, joined with `::`. Empty when no namespace is declared. |
| `name` | Packet declaration name. |
| `qualified_name` | Namespace plus packet name, or just `name` when no namespace is declared. |
| `kind` | Packet kind: `command` or `telemetry`. |
| `source` | Source `.syn` file that declared the packet. |
| `topic` | Logical command-group or telemetry topic name. |
| `cc` | Resolved numeric command code for commands. `null` in JSON and empty in CSV for telemetry. |

## Intended Uses

Registry output is useful for:

- Mission packet tables.
- ICD generation.
- Database ingestion.
- Review artifacts.
- Dashboard or ground-tool inputs.
- CI reports that need a simple packet inventory.

The registry intentionally stays small. If another system needs storage, search, dashboards, or mission operations workflows, it can consume the exported JSON or CSV without Synapse becoming that system.

## Current Stability

The registry fields documented above are the `0.3.x` packet registry shape.
Future versions may add fields for original symbolic command-code expressions,
field-level schemas, ownership metadata, or schema versioning.
