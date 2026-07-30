# Synapse 0.3 Quick Start

This guide walks through the complete Synapse workflow: define reusable cFS
message interfaces, assign mission-owned topic IDs, validate the mission, and
generate both packet bindings and a cFE routing header.

The commands below run from a Synapse source checkout and use the checked-in
[`mission-demo`](../examples/mission-demo) schemas. If `synapse` is already
installed, replace `cargo run -p cfs-synapse --` with `synapse`.

## 0.3 Support Scope

Synapse 0.3 targets standard, non-EDS cFS builds. Generated C headers are the
primary flight-integration output. The application still owns its cFE
lifecycle and calls APIs such as `CFE_MSG_Init`, `CFE_SB_Subscribe`,
`CFE_SB_ReceiveBuffer`, and `CFE_SB_TransmitMsg`.

Generated Rust is intentionally limited to `#[repr(C)]` ABI types. Synapse and
the repository's `cfs-sys` helper do not provide Rust wrappers for cFE runtime
APIs or a Rust application framework.

EDS-enabled builds are outside the 0.3 scope because EDS generates and owns the
component interface headers.

## 1. Define Logical Topics

A `.syn` schema owns packet structure and logical topic names. Commands belong
to a `commands` group and use `@cc(...)` function codes:

```syn
namespace camera_app

enum u8 CameraMode {
    Standby = 0
    Imaging = 1
    Safe = 2
}

telemetry CameraStatus {
    mode: CameraMode
    frames_captured: u32
    detector_temp_c: f32
}

commands CameraCommands {
    @cc(1)
    command SetCameraMode {
        mode: CameraMode
    }
}
```

The logical topics are:

- `camera_app::CameraCommands`
- `camera_app::CameraStatus`

Do not put `@mid(...)` in a schema. Runtime message-ID mapping belongs to the
mission configuration.

## 2. Assign Mission Topic IDs

The human-owned `mission.toml` assigns each logical topic a cFE topic ID:

```toml
version = 1

[topics.command]
"camera_app::CameraCommands" = 0x82

[topics.telemetry]
"camera_app::CameraStatus" = 0x82
```

Command and telemetry assignments are separate ID spaces, so the same numeric
topic ID may appear once in each section. Synapse reads this file but never
modifies it.

The checked-in demo manifest also assigns navigation and payload topics.

## 3. Validate the Complete Mission

Pass the manifest and every mission-visible schema root:

```bash
cargo run -p cfs-synapse -- check \
  --manifest examples/mission-demo/mission.toml \
  examples/mission-demo/syn/nav_app.syn \
  examples/mission-demo/syn/camera_app.syn \
  examples/mission-demo/syn/payload_app.syn
```

A successful command reports each checked root. Validation catches missing or
stale manifest assignments, topics placed in the wrong section, duplicate
topic IDs, duplicate logical telemetry topics, duplicate command function
codes, import errors, and unsupported ABI constructs.

## 4. Generate Packet Bindings

Generate C:

```bash
cargo run -p cfs-synapse -- generate \
  --lang c \
  -o /tmp/synapse-quick-start/c \
  examples/mission-demo/syn/camera_app.syn
```

Generate Rust:

```bash
cargo run -p cfs-synapse -- generate \
  --lang rust \
  -o /tmp/synapse-quick-start/rust \
  examples/mission-demo/syn/camera_app.syn
```

These commands write `camera_app.h` and `camera_app.rs`. The generated packet
types contain the appropriate cFS command or telemetry header. Command
bindings include `_CC` constants; they do not contain deployment MID constants.

## 5. Generate the cFE Routing Header

Generate mappings for the complete manifest:

```bash
cargo run -p cfs-synapse -- routes \
  --manifest examples/mission-demo/mission.toml \
  -o /tmp/synapse-quick-start/mission_topics.h \
  examples/mission-demo/syn/nav_app.syn \
  examples/mission-demo/syn/camera_app.syn \
  examples/mission-demo/syn/payload_app.syn
```

The routing header maps mission topic IDs through the cFE platform macros:

```c
#include "cfe_core_api_msgid_mapping.h"

#define CAMERA_APP_CAMERA_COMMANDS_TOPICID  0x0082U
#define CAMERA_APP_CAMERA_COMMANDS_MID      CFE_PLATFORM_CMD_TOPICID_TO_MIDV(CAMERA_APP_CAMERA_COMMANDS_TOPICID)
```

At a cFE API boundary, convert the generated value with the normal cFE API:

```c
CFE_SB_Subscribe(
    CFE_SB_ValueToMsgId(CAMERA_APP_CAMERA_COMMANDS_MID),
    CommandPipe
);
```

## 6. Use the Generated C Type in a cFS Application

Include both generated artifacts and initialize a telemetry packet through the
cFE Message API:

```c
#include "camera_app.h"
#include "mission_topics.h"

camera_app_CameraStatus_t CameraStatus;

CFE_MSG_Init(
    CFE_MSG_PTR(CameraStatus.Header),
    CFE_SB_ValueToMsgId(CAMERA_APP_CAMERA_STATUS_MID),
    sizeof(CameraStatus)
);
```

Fill the generated payload fields, timestamp the packet, and transmit it
through the Software Bus:

```c
CameraStatus.mode            = CAMERA_APP_CAMERA_MODE_IMAGING;
CameraStatus.frames_captured = FramesCaptured;
CameraStatus.detector_temp_c = DetectorTempC;

CFE_SB_TimeStampMsg(CFE_MSG_PTR(CameraStatus.Header));
CFE_SB_TransmitMsg(CFE_MSG_PTR(CameraStatus.Header), true);
```

Always use cFE Message and Software Bus APIs to initialize, inspect, and modify
header fields. Synapse generates the packet layout and routing constants; it
does not generate an application main loop, command dispatcher, or error
handling. Production application code must check the status returned by each
cFE API call.

## 7. Optional Documentation and Registry Outputs

Generate searchable HTML documentation:

```bash
cargo run -p cfs-synapse -- doc \
  -o /tmp/synapse-quick-start/docs \
  examples/mission-demo/syn/camera_app.syn
```

Export a JSON packet inventory:

```bash
cargo run -p cfs-synapse -- registry \
  --format json \
  -o /tmp/synapse-quick-start/packets.json \
  examples/mission-demo/syn/camera_app.syn
```

The registry contains packet names, logical topics, and command codes. Final
cFE MsgId values remain mission/platform-owned and are not registry fields.

## Rules to Remember

- Put every command inside a `commands` group.
- Give every command a group-unique `@cc(...)`.
- Treat each telemetry declaration as one logical telemetry topic.
- Keep `@mid(...)` out of `.syn` files.
- Update `mission.toml` manually when logical topics are added, removed, or
  reassigned.
- Validate the complete mission schema set when using `--manifest`.
- Commit the schema, manifest, and generated routing header together when your
  project checks generated artifacts into source control.
- Use cFE APIs rather than accessing generated message-header bits directly.

For more detail, see the [language reference](language.md), the
[mission-routing guide](mission.md), and the
[routing architecture](routing-model.md). To reproduce the real cFS 7.0.1
command and telemetry validation, follow the
[cFS integration test](cfs-integration-test.md).
