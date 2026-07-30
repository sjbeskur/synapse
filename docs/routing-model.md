# Synapse Routing Model

Status: implemented for logical-topic validation and C routing-header generation.

## Boundary

Synapse schema files define reusable cFS message interfaces. They do not assign
deployment-specific Software Bus Message IDs.

- A `telemetry` declaration defines one logical telemetry topic.
- A `commands` group defines one logical command topic.
- Commands inside a group are selected by cFE function code.
- A mission manifest assigns numeric topic IDs to logical topics.
- cFE mission/platform configuration maps topic IDs to final MsgId values.

Message IDs remain mandatory in deployed cFS software. They are generated from
mission-owned topic assignments instead of being embedded in reusable schemas.

## Schema Example

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

telemetry CameraStatus {
    mode: u8
}
```

The logical topics are `camera_app::CameraCommands` and
`camera_app::CameraStatus`.

## Mission Manifest Example

```toml
version = 1

[topics.command]
"camera_app::CameraCommands" = 0x82

[topics.telemetry]
"camera_app::CameraStatus" = 0x83
```

Command and telemetry topic ID spaces are validated separately. An equal
numeric topic ID may appear once in each space because cFE uses distinct
command and telemetry mappings.

## Generation Modes

Structure-only generation does not require a mission manifest. It emits ABI
types and command function codes.

Deployable routing generation requires a mission manifest. It emits topic
definitions and MsgId mapping macros, and fails when an assignment is missing,
stale, duplicated, or placed in the wrong command/telemetry section.

Generation never edits the mission manifest, so existing assignments cannot be
renumbered silently.

Validate a complete mission-visible schema set:

```bash
synapse check --manifest mission.toml \
  schemas/camera.syn schemas/navigation.syn
```

Generate a standalone routing header:

```bash
synapse routes --manifest mission.toml \
  -o generated/mission_topics.h \
  schemas/camera.syn schemas/navigation.syn
```

## C Mapping

Generated deployment headers use cFE's mission mapping:

```c
#include "cfe_core_api_msgid_mapping.h"

#define CAMERA_APP_CAMERA_COMMANDS_TOPICID 0x0082U
#define CAMERA_APP_CAMERA_COMMANDS_MID \
    CFE_PLATFORM_CMD_TOPICID_TO_MIDV(CAMERA_APP_CAMERA_COMMANDS_TOPICID)
```

Applications convert the integer MsgId value at an API boundary:

```c
CFE_SB_Subscribe(
    CFE_SB_ValueToMsgId(CAMERA_APP_CAMERA_COMMANDS_MID),
    CommandPipe
);
```

Synapse does not interpret MsgId bits or select `MISSION_MSG_V1` versus
`MISSION_MSG_V2`; that is owned by cFE mission/platform configuration.

Synapse 0.3 generates this integration for standard, non-EDS cFS builds.
EDS-enabled builds own interface-header generation and are not supported by
this release.
