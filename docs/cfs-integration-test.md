# Synapse 0.3 cFS Integration Test

This guide reproduces the end-to-end integration test used to validate
Synapse-generated C messages with a standard, non-EDS NASA cFS 7.0.1 build.
It creates a small cFS application that:

- Publishes a Synapse-generated telemetry packet.
- Subscribes to a mission-owned command topic.
- Receives and dispatches a Synapse-generated `Noop` command through
  `CI_LAB`.

The final data paths are:

```text
synapse_demo app
  -> generated Status type
  -> mission-generated telemetry MID
  -> cFS Software Bus

cmdUtil
  -> CI_LAB
  -> cFS Software Bus
  -> mission-generated command MID
  -> generated Noop command code and type
  -> synapse_demo app
```

This is an integration test, not an application framework. Synapse generates
packet layouts, command codes, and routing constants. The application still
owns its cFE lifecycle, Software Bus pipes, dispatch, error handling, and
scheduling.

## Tested Configuration

- NASA cFS bundle tag: `v7.0.1`
- Build: `native_std`
- Interface model: standard non-EDS cFS
- Synapse: `0.3.0`
- Host: Linux

The test uses topic ID `0x93` because it is unused in both topic spaces in the
`v7.0.1` sample mission. Do not copy that assignment into another mission
without checking that mission's topic-ID registry.

## 1. Install Synapse

Install a released `cfs-synapse` package:

```bash
cargo install cfs-synapse
synapse --help
```

To test a Synapse source checkout instead:

```bash
cd /path/to/synapse
cargo install --path synapse --locked --force
synapse --help
```

## 2. Create and Build the cFS Lab

Clone the pinned cFS bundle and all of its submodules:

```bash
cd /path/to/your/projects
git clone --branch v7.0.1 --recurse-submodules \
  https://github.com/nasa/cFS.git cfs-synapse-lab
cd cfs-synapse-lab
git switch -c lab/synapse-demo
```

The clone is pinned to a tag, so creating a branch before editing the mission
avoids committing from a detached `HEAD`.

Build and install the standard native configuration:

```bash
CMAKE_POLICY_VERSION_MINIMUM=3.5 make native_std.install
```

The `CMAKE_POLICY_VERSION_MINIMUM` setting is needed when a recent CMake
version rejects the older minimum declared by the bundled SBN app. See
[Troubleshooting](#troubleshooting) if the build instead stops on the PSP
RTEMS header guard.

Run the unmodified mission once:

```bash
cd build-native_std/exe/cpu1
./core-cpu1
```

Stop it with `Ctrl-C` after cFS completes startup. Return to the cFS root:

```bash
cd ../../..
```

## 3. Create the Demo Application

Create this layout inside the cFS checkout:

```text
apps/synapse_demo/
├── CMakeLists.txt
├── fsw/
│   ├── inc/
│   └── src/
│       └── synapse_demo_app.c
├── generated/
└── syn/
    └── synapse_demo.syn
```

The application may be mission-owned, maintained as a separate repository, or
included as a submodule. The integration test used a separate Git repository
at `apps/synapse_demo`, but its repository arrangement does not affect the cFS
build.

Create `apps/synapse_demo/CMakeLists.txt`:

```cmake
project(CFE_SYNAPSE_DEMO C)

add_cfe_app(
  synapse_demo
  fsw/src/synapse_demo_app.c
)

target_include_directories(
  synapse_demo
  PUBLIC
    fsw/inc
    generated
)
```

The application source will be added after generating its message types.

## 4. Define the Application Interface

Create `apps/synapse_demo/syn/synapse_demo.syn`:

```syn
namespace synapse_demo

/// Periodic status published by the demo application.
telemetry Status {
    /// Number of status packets produced.
    counter: u32
}

/// Commands accepted by the demo application.
commands Commands {
    /// Verify that the application can receive a command.
    @cc(0)
    command Noop {
    }
}
```

The schema deliberately contains no MID. It owns the logical topic names,
packet layout, and command code; the mission owns deployment routing.

Check the schema and generate its C packet header:

```bash
synapse check apps/synapse_demo/syn/synapse_demo.syn

synapse generate \
  --lang c \
  -o apps/synapse_demo/generated \
  apps/synapse_demo/syn/synapse_demo.syn
```

This writes `apps/synapse_demo/generated/synapse_demo.h`. It contains:

- `synapse_demo_Status_t` with `CFE_MSG_TelemetryHeader_t` first.
- `synapse_demo_Noop_t` with `CFE_MSG_CommandHeader_t` first.
- `NOOP_CC` with value `0`.

It does not contain a deployment MID.

## 5. Assign Mission Topic IDs

Create the human-owned `sample_defs/mission.toml`:

```toml
version = 1

[topics.command]
"synapse_demo::Commands" = 0x93

[topics.telemetry]
"synapse_demo::Status" = 0x93
```

Command and telemetry topics occupy separate ID spaces, so the same numeric
topic ID may occur once in each section.

Validate the complete manifest and schema set:

```bash
synapse check \
  --manifest sample_defs/mission.toml \
  apps/synapse_demo/syn/synapse_demo.syn
```

Synapse never modifies `mission.toml`; topic assignments remain explicit,
reviewable mission changes. cFS does not read this TOML file directly. The
generated routing header is the C build input.

When a mission contains more than one Synapse schema root, list all of them in
the same `check --manifest` and `routes --manifest` commands.

## 6. Generate the Mission Routing Header

Generate the mission-owned routing header into the sample mission's include
directory:

```bash
synapse routes \
  --manifest sample_defs/mission.toml \
  -o sample_defs/inc/synapse_demo_topics.h \
  apps/synapse_demo/syn/synapse_demo.syn
```

The relevant output is:

```c
#include "cfe_core_api_msgid_mapping.h"

#define SYNAPSE_DEMO_COMMANDS_TOPICID  0x0093U
#define SYNAPSE_DEMO_COMMANDS_MID      CFE_PLATFORM_CMD_TOPICID_TO_MIDV(SYNAPSE_DEMO_COMMANDS_TOPICID)

#define SYNAPSE_DEMO_STATUS_TOPICID  0x0093U
#define SYNAPSE_DEMO_STATUS_MID      CFE_PLATFORM_TLM_TOPICID_TO_MIDV(SYNAPSE_DEMO_STATUS_TOPICID)
```

The logical topic IDs happen to match, but the mapping functions do not:
command and telemetry MIDs are constructed from different mission-configured
bases.

At cFE API boundaries, convert the generated MID value with
`CFE_SB_ValueToMsgId(...)`.

## 7. Implement the cFS Application

Create `apps/synapse_demo/fsw/src/synapse_demo_app.c`:

```c
#include "cfe.h"

#include "synapse_demo.h"
#include "synapse_demo_topics.h"

#define SYNAPSE_DEMO_INIT_INF_EID    1
#define SYNAPSE_DEMO_TLM_INF_EID     2
#define SYNAPSE_DEMO_TLM_ERR_EID     3
#define SYNAPSE_DEMO_CMD_ERR_EID     4
#define SYNAPSE_DEMO_NOOP_INF_EID    5
#define SYNAPSE_DEMO_CMD_MSG_ERR_EID 6

#define SYNAPSE_DEMO_CMD_PIPE_DEPTH 4
#define SYNAPSE_DEMO_CMD_PIPE_NAME  "SYN_DEMO_CMD"

static synapse_demo_Status_t SYNAPSE_DEMO_Status;
static CFE_SB_PipeId_t       SYNAPSE_DEMO_CommandPipe;

static void SYNAPSE_DEMO_ProcessCommand(const CFE_SB_Buffer_t *buffer)
{
    CFE_MSG_FcnCode_t command_code = 0;
    CFE_MSG_Size_t    actual_size  = 0;

    CFE_MSG_GetFcnCode(&buffer->Msg, &command_code);
    CFE_MSG_GetSize(&buffer->Msg, &actual_size);

    if (command_code != NOOP_CC)
    {
        CFE_EVS_SendEvent(
            SYNAPSE_DEMO_CMD_MSG_ERR_EID,
            CFE_EVS_EventType_ERROR,
            "Unknown command code: %u",
            (unsigned int)command_code
        );
    }
    else if (actual_size != sizeof(synapse_demo_Noop_t))
    {
        CFE_EVS_SendEvent(
            SYNAPSE_DEMO_CMD_MSG_ERR_EID,
            CFE_EVS_EventType_ERROR,
            "Invalid Noop length: %lu, expected %lu",
            (unsigned long)actual_size,
            (unsigned long)sizeof(synapse_demo_Noop_t)
        );
    }
    else
    {
        CFE_EVS_SendEvent(
            SYNAPSE_DEMO_NOOP_INF_EID,
            CFE_EVS_EventType_INFORMATION,
            "Synapse Noop command received"
        );
    }
}

void SYNAPSE_DEMO_Main(void)
{
    CFE_Status_t    status;
    CFE_SB_Buffer_t *command_buffer = NULL;
    uint32          run_status      = CFE_ES_RunStatus_APP_RUN;

    status = CFE_EVS_Register(NULL, 0, CFE_EVS_EventFilter_BINARY);

    if (status != CFE_SUCCESS)
    {
        CFE_ES_WriteToSysLog(
            "Synapse Demo: event registration failed, RC = 0x%08lX\n",
            (unsigned long)status
        );

        run_status = CFE_ES_RunStatus_APP_ERROR;
    }
    else
    {
        status = CFE_MSG_Init(
            CFE_MSG_PTR(SYNAPSE_DEMO_Status.Header),
            CFE_SB_ValueToMsgId(SYNAPSE_DEMO_STATUS_MID),
            sizeof(SYNAPSE_DEMO_Status)
        );

        if (status != CFE_SUCCESS)
        {
            CFE_EVS_SendEvent(
                SYNAPSE_DEMO_TLM_ERR_EID,
                CFE_EVS_EventType_ERROR,
                "Status packet initialization failed, RC = 0x%08lX",
                (unsigned long)status
            );

            run_status = CFE_ES_RunStatus_APP_ERROR;
        }
        else
        {
            status = CFE_SB_CreatePipe(
                &SYNAPSE_DEMO_CommandPipe,
                SYNAPSE_DEMO_CMD_PIPE_DEPTH,
                SYNAPSE_DEMO_CMD_PIPE_NAME
            );

            if (status == CFE_SUCCESS)
            {
                status = CFE_SB_Subscribe(
                    CFE_SB_ValueToMsgId(SYNAPSE_DEMO_COMMANDS_MID),
                    SYNAPSE_DEMO_CommandPipe
                );
            }

            if (status != CFE_SUCCESS)
            {
                CFE_EVS_SendEvent(
                    SYNAPSE_DEMO_CMD_ERR_EID,
                    CFE_EVS_EventType_ERROR,
                    "Command pipe setup failed, RC = 0x%08lX",
                    (unsigned long)status
                );

                run_status = CFE_ES_RunStatus_APP_ERROR;
            }
            else
            {
                CFE_EVS_SendEvent(
                    SYNAPSE_DEMO_INIT_INF_EID,
                    CFE_EVS_EventType_INFORMATION,
                    "Synapse Demo initialized and subscribed"
                );
            }
        }
    }

    while (CFE_ES_RunLoop(&run_status))
    {
        status = CFE_SB_ReceiveBuffer(
            &command_buffer,
            SYNAPSE_DEMO_CommandPipe,
            CFE_SB_POLL
        );

        if (status == CFE_SUCCESS)
        {
            SYNAPSE_DEMO_ProcessCommand(command_buffer);
        }
        else if (status != CFE_SB_NO_MESSAGE)
        {
            CFE_EVS_SendEvent(
                SYNAPSE_DEMO_CMD_MSG_ERR_EID,
                CFE_EVS_EventType_ERROR,
                "Command pipe receive failed, RC = 0x%08lX",
                (unsigned long)status
            );

            run_status = CFE_ES_RunStatus_APP_ERROR;
        }

        ++SYNAPSE_DEMO_Status.counter;
        CFE_SB_TimeStampMsg(CFE_MSG_PTR(SYNAPSE_DEMO_Status.Header));

        status = CFE_SB_TransmitMsg(
            CFE_MSG_PTR(SYNAPSE_DEMO_Status.Header),
            true
        );

        if (status != CFE_SUCCESS)
        {
            CFE_EVS_SendEvent(
                SYNAPSE_DEMO_TLM_ERR_EID,
                CFE_EVS_EventType_ERROR,
                "Status packet transmission failed, RC = 0x%08lX",
                (unsigned long)status
            );

            run_status = CFE_ES_RunStatus_APP_ERROR;
        }
        else if (SYNAPSE_DEMO_Status.counter == 1U)
        {
            CFE_EVS_SendEvent(
                SYNAPSE_DEMO_TLM_INF_EID,
                CFE_EVS_EventType_INFORMATION,
                "First Synapse status packet published"
            );
        }

        OS_TaskDelay(1000);
    }

    CFE_ES_ExitApp(run_status);
}
```

The one-second poll loop is intentionally simple and observable. A production
application should use its mission's scheduling model and application
architecture.

## 8. Add the App to the Sample Mission

In `sample_defs/targets.cmake`, append the app to the global application list:

```cmake
list(APPEND MISSION_GLOBAL_APPLIST synapse_demo)
```

In `sample_defs/generate_startup.cmake`, add the app to the startup script
written by `generate_cfs_startup_script`:

```cmake
"CFE_APP, synapse_demo, SYNAPSE_DEMO_Main, SYNAPSE_DEMO, 55, 32768, 0x0, 0;\n"
```

Place it alongside the existing `CFE_APP` strings inside the `file(WRITE ...)`
call. Keep it as its own quoted string.

The mission now owns:

- Whether `synapse_demo` is built.
- Whether and how it starts.
- `sample_defs/mission.toml`.
- `sample_defs/inc/synapse_demo_topics.h`.

The application owns:

- `syn/synapse_demo.syn`.
- `generated/synapse_demo.h`.
- Its C source and build definition.

## 9. Build and Run the Integrated Mission

From the cFS root:

```bash
CMAKE_POLICY_VERSION_MINIMUM=3.5 make native_std.install
```

Run CPU 1:

```bash
cd build-native_std/exe/cpu1
./core-cpu1
```

Successful application initialization and telemetry publication produce:

```text
Synapse Demo initialized and subscribed
First Synapse status packet published
```

The second event means cFS accepted a real packet created from:

- The generated `synapse_demo_Status_t` layout.
- The mission-generated telemetry MID.
- `CFE_MSG_Init`.
- `CFE_SB_TimeStampMsg`.
- `CFE_SB_TransmitMsg`.

Leave `core-cpu1` running for the command test.

## 10. Send the Generated Noop Command

The sample mission's CPU 1 command base is `0x1800`. The assigned command
topic is `0x93`, so the deployed command MID is:

```text
0x1800 + 0x0093 = 0x1893
```

In another terminal, build the command utility included with cFS:

```bash
cd /path/to/cfs-synapse-lab/tools/cFS-GroundSystem/Subsystems/cmdUtil
make
```

Send function code `0`, which matches generated `NOOP_CC`:

```bash
./cmdUtil \
  --host=127.0.0.1 \
  --port=1234 \
  --pktid=0x1893 \
  --cmdcode=0
```

The running cFS process should report:

```text
Synapse Noop command received
```

Send the command twice if desired. Because this integration app polls once per
second, two queued commands appear as two events approximately one second
apart.

This event validates the complete command path:

```text
cmdUtil
  -> CI_LAB UDP ingest
  -> cFS Software Bus
  -> SYNAPSE_DEMO_COMMANDS_MID subscription
  -> NOOP_CC dispatch
  -> sizeof(synapse_demo_Noop_t) validation
```

## Success Criteria

The integration test passes when all three events appear:

```text
Synapse Demo initialized and subscribed
First Synapse status packet published
Synapse Noop command received
```

This proves that Synapse 0.3 C packet types and mission-owned routing constants
work with the cFS 7.0.1 standard message and Software Bus APIs.

It does not validate EDS builds, a Rust cFS runtime, mission scheduling design,
production command/telemetry ground-system configuration, or decoding the
`Status` packet at a ground endpoint. The telemetry assertion in this test is
that `CFE_SB_TransmitMsg` accepts the initialized packet.

## Troubleshooting

### CMake rejects SBN's old minimum version

Recent CMake releases may report:

```text
Compatibility with CMake < 3.5 has been removed from CMake.
```

Run the cFS build with:

```bash
CMAKE_POLICY_VERSION_MINIMUM=3.5 make native_std.install
```

This was sufficient for the pinned integration checkout.

### GCC reports a PSP RTEMS header-guard error

GCC 15 may stop the build because warnings are treated as errors:

```text
header guard 'OVERRIDE_TOOIMPL_H' followed by '#define' of a different macro
```

In:

```text
psp/unit-test-coverage/ut-stubs/override_inc/rtems/score/todimpl.h
```

change:

```c
#ifndef OVERRIDE_TOOIMPL_H
```

to:

```c
#ifndef OVERRIDE_TODIMPL_H
```

The following `#define OVERRIDE_TODIMPL_H` is already correct. Re-run the
build afterward. This is a host-toolchain compatibility fix in the pinned PSP
submodule, not a Synapse-generated code change.

### cFS starts but the demo app does not

Confirm that:

- `synapse_demo` appears in `MISSION_GLOBAL_APPLIST`.
- `generate_startup.cmake` contains its `CFE_APP` line.
- `build-native_std/exe/cpu1/cf/synapse_demo.so` exists.
- `build-native_std/exe/cpu1/cf/cfe_es_startup.scr` contains
  `synapse_demo`.

Re-run `native_std.install` after changing either mission CMake file.

### `cmdUtil` sends but no Noop event appears

Confirm that:

- `CI_LAB` started successfully.
- `core-cpu1` is still running.
- `cmdUtil` sends to UDP port `1234`.
- The sample mission still uses command base `0x1800`.
- `mission.toml` still assigns `synapse_demo::Commands` to `0x93`.
- The app subscribes through
  `CFE_SB_ValueToMsgId(SYNAPSE_DEMO_COMMANDS_MID)`.

For another mission, derive the deployed MID from that mission's cFE mapping
configuration instead of assuming `0x1893`.

## Source-Control Boundaries

If the application is a separate repository, commit its schema, generated
packet header, source, and `CMakeLists.txt` there.

Commit the manifest, generated routing header, application-list change, and
startup-script change in the mission repository. Do not accidentally commit
an unrelated dirty cFS submodule pointer or the `build-native_std` output.
