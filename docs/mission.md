# Mission-Wide Routing

Synapse separates reusable message interfaces from mission deployment
assignments:

- `.syn` files define logical command and telemetry topics.
- `mission.toml` assigns cFE topic IDs.
- cFE mission/platform configuration maps those topic IDs to final MsgId
  values.

This keeps deployment-specific numbers out of reusable schemas while still
making the complete routing configuration reviewable and deterministic.

See [`routing-model.md`](routing-model.md) for the architectural boundary.

## Schema Topics

A command group defines one logical command topic. Commands inside it are
selected by function code:

```synapse
namespace camera_app

commands CameraCommands {
    @cc(1)
    command SetMode {
        mode: u8
    }

    @cc(2)
    command SetExposure {
        exposure_us: u32
    }
}
```

Each telemetry declaration defines one logical telemetry topic:

```synapse
telemetry CameraStatus {
    mode: u8
}
```

The fully qualified topics are `camera_app::CameraCommands` and
`camera_app::CameraStatus`.

## Mission Manifest

The manifest is a human-owned TOML file:

```toml
version = 1

[topics.command]
"camera_app::CameraCommands" = 0x82

[topics.telemetry]
"camera_app::CameraStatus" = 0x83
```

Command and telemetry topic IDs occupy separately validated spaces. The same
numeric ID may therefore occur once in each section.

Synapse reads but never modifies this file. Assignment changes remain explicit
source-control changes.

## Validation

Validate all mission-visible schema roots together:

```bash
synapse check --manifest mission.toml \
  schemas/navigation.syn \
  schemas/camera.syn \
  schemas/payload.syn
```

Synapse loads and deduplicates their import closures before checking:

- Missing logical-topic assignments.
- Stale manifest assignments with no matching schema topic.
- Command topics placed under `topics.telemetry`, or the reverse.
- Duplicate numeric IDs within one topic space.
- Duplicate function codes within a command topic.
- Duplicate logical telemetry declarations.
- C macro-name collisions after namespace/name normalization.
- Unsupported schema and ABI constructs.

Manifest validation is intentionally strict. Pass the complete mission-visible
schema set rather than a subset when the manifest describes the whole mission.

## Routing Header

Generate the cFE routing header:

```bash
synapse routes --manifest mission.toml \
  -o generated/mission_topics.h \
  schemas/navigation.syn \
  schemas/camera.syn \
  schemas/payload.syn
```

Without `-o`, the header is written to stdout. The output includes
`cfe_core_api_base_msgids.h` and maps topic IDs using the mission-configured
cFE macros:

```c
#define CAMERA_APP_CAMERA_COMMANDS_TOPICID  0x0082U
#define CAMERA_APP_CAMERA_COMMANDS_MID \
    CFE_PLATFORM_CMD_TOPICID_TO_MIDV(CAMERA_APP_CAMERA_COMMANDS_TOPICID)

#define CAMERA_APP_CAMERA_STATUS_TOPICID  0x0083U
#define CAMERA_APP_CAMERA_STATUS_MID \
    CFE_PLATFORM_TLM_TOPICID_TO_MIDV(CAMERA_APP_CAMERA_STATUS_TOPICID)
```

At a cFE API boundary, convert the message-ID value using the normal cFE API:

```c
CFE_SB_Subscribe(
    CFE_SB_ValueToMsgId(CAMERA_APP_CAMERA_COMMANDS_MID),
    CommandPipe
);
```

## Other Mission Outputs

The same roots can produce documentation and a machine-readable registry:

```bash
synapse doc -o generated/docs schemas/navigation.syn schemas/camera.syn
synapse registry --format json -o generated/packets.json \
  schemas/navigation.syn schemas/camera.syn
```

The registry reports logical topics and command codes. Final MsgId values are
not registry fields because they belong to the cFE mission/platform mapping.

## Schema Boundary

Schema-level `@mid(...)` attributes and top-level `command` declarations are
rejected. Commands must be nested in a `commands` group, telemetry declarations
name their own logical topic, and every mission-visible topic must be assigned
by the manifest.
