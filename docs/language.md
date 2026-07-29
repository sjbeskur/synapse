# Synapse Language Status

This document records the current Synapse IDL surface for cFS code generation.

See `docs/examples.md` for current `.syn` examples and generated output links.
See `docs/roadmap-0.2.md` for the active `0.2.x` language review.

## Release Posture

`0.1.x` should be treated as a stabilization and inventory release:

- Keep the current syntax mostly intact.
- Document what code generation supports today.
- Mark parsed-only features clearly.
- Avoid large language redesigns before the first publish.
- Prefer conservative behavior over broad promises.

`0.2.x` is the right place for deliberate IDL changes after the first release has real usage feedback.

## Stable For 0.1

These features are part of the intended `0.1.x` authoring path.

### `namespace`

```syn
namespace camera_app
```

Namespaces are used by C codegen to prefix generated type names, such as `camera_app_CameraStatus_t`. Rust codegen currently keeps Rust type names unprefixed and uses Rust module structure/imports for organization.

### `import`

```syn
import "std_msgs.syn"
```

Imports generate C `#include` lines and Rust `use crate::...` lines. Path-based generation through the CLI or `generate_file` validates the import graph rooted at the input file: imported files must exist, parse, and provide any referenced qualified types such as `std_msgs::Header`.

Imported type references should be namespace-qualified. For example, after `import "std_msgs.syn"`, use `std_msgs::Header` rather than bare `Header`. Bare type references are reserved for declarations in the current file.

Each file can reference only its local declarations and directly imported namespaces. Transitive imports are loaded and validated so dependency files are checked, but a root file must directly import any namespace it references. CLI output-directory generation emits the root file plus its transitive imports in dependency order by default; add `--single-file` to emit only the requested root file. In library code, use `generate_files` for the import closure and `generate_file` for only the root. Integer constants from directly imported namespaces may be used in attributes such as `@cc(nav_app::SET_MODE_CC)`.

### `struct`

```syn
struct Point {
    x: f64
    y: f64
}
```

Plain structs generate ABI-compatible C structs and Rust `#[repr(C)]` structs without cFS Software Bus headers.

### `enum`

```syn
enum u8 CameraMode {
    Standby = 0
    Preview = 1
}
```

Represented enums generate fixed-width integer aliases and named constants. In C, variant macros include namespace ownership when the file has a namespace, for example `CAMERA_APP_CAMERA_MODE_STANDBY`. The representation must be an integer primitive: `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, or `u64`. Every variant in a represented enum must have an explicit value, and values must fit the selected representation.

Unrepresented enums still parse, but cFS codegen rejects them when used as field types because they do not define an ABI width.

### `commands` and `command`

```syn
commands CameraCommands {
    @cc(1)
    command SetMode {
        mode: CameraMode
    }
}
```

A `commands` group defines one logical command topic. Its nested commands
generate Software Bus packet structs with `CFE_MSG_CommandHeader_t` as the
first C field and `cfs_sys::CFE_MSG_CommandHeader_t` as the first Rust field.
Each command requires an `@cc(...)`, unique within the group. The topic ID is
assigned in the mission manifest rather than the schema.

### `telemetry`

```syn
telemetry NavState {
    x: f64
    y: f64
}
```

Each telemetry declaration defines one logical telemetry topic. Telemetry
packets generate Software Bus packet structs with `CFE_MSG_TelemetryHeader_t`
as the first C field and `cfs_sys::CFE_MSG_TelemetryHeader_t` as the first Rust
field. The topic ID is assigned in the mission manifest.

### `table`

```syn
table NavConfig {
    max_speed: f64
    enabled: bool
}
```

Tables generate plain data structs without cFS Software Bus headers. They are intended for cFS Table Services payload data, not table-management commands.

### Mission Topic Assignments

```toml
version = 1

[topics.command]
"camera_app::CameraCommands" = 0x82

[topics.telemetry]
"nav_app::NavState" = 0x83
```

Use `synapse check --manifest mission.toml ...` to validate completeness and
`synapse routes --manifest mission.toml ...` to emit cFE topic-ID and MsgId
mapping macros. Command and telemetry topic-ID spaces are checked separately.
Synapse reads but never modifies the manifest.

Top-level `command` declarations and schema-level `@mid(...)` attributes are
rejected. This keeps deployment routing out of reusable schemas.

### Primitive Types

Supported primitive names:

```text
f32 f64
i8 i16 i32 i64
u8 u16 u32 u64
bool
bytes
```

These map to fixed C integer/float types and Rust primitive types where possible.

See `docs/types.md` for the full scalar, array, bounded array, and string mapping reference.

### Fixed Arrays

```syn
struct CameraIntrinsics {
    k: f64[9]
}
```

Fixed arrays generate inline storage in C and Rust.

### Bounded Strings

```syn
struct CameraId {
    name: string[<=32]
}
```

Bounded strings generate inline `char[N]` storage in C and `[u8; N]` storage in Rust. The bound is the exact storage size in bytes. Synapse does not guarantee null termination, text encoding, or that one byte equals one character.

Projects may adopt a C-string convention for these buffers. If they do, the bound includes the null terminator, the usable text capacity is `N - 1`, and readers should scan for the first null byte with an upper bound of `N`. Helper functions for Rust and C++ string views are a good fit for a support library rather than generated packet fields.

### Documentation Comments

```syn
/// Stable identifier for one camera.
struct CameraId {
    /// Mission-defined camera name.
    name: string[<=32]
}
```

`///` doc comments are parsed and emitted as generated documentation comments for supported declarations and fields. `//` comments are ordinary comments and are not emitted.

## Supported With Caveats

These features are accepted by the parser, but cFS codegen rejects some forms until their ABI behavior is explicit.

### Dynamic Arrays

```syn
struct Polygon {
    points: Point32[]
}
```

Dynamic arrays parse into the AST, but cFS codegen rejects them because the IDL does not yet define an ownership or length model.

### Bounded Dynamic Arrays

```syn
struct Samples {
    values: f32[<=128]
}
```

Bounded arrays parse into the AST. `string[<=N]` is supported as inline storage, but cFS codegen rejects non-string bounded arrays until an inline storage plus length-field policy exists.

### Unbounded Strings

```syn
struct Frame {
    name: string
}
```

Unbounded strings parse into the AST, but cFS codegen rejects them because they would require pointer-like ABI fields. Use `string[<=N]` or `string[N]` for ABI-stable cFS packet and table payloads, and define the mission/application encoding and termination policy outside the generated type.

### `const`

```syn
const MAX_CAMERAS: u8 = 4
```

Constants generate C `#define`s and Rust `pub const`s. Integer constants can
be used by local or directly importing files in `@cc(...)` attributes.
Transitive-only constants are not visible unless the file imports their
namespace directly.

### Legacy `message`

```syn
message NavState {
    x: f64
}
```

`message` remains accepted by the parser for older files and possible non-cFS backends, but cFS codegen rejects it.

Use explicit `command` or `telemetry` for generated cFS packets.

## Parsed But Not Fully Generated

These features are accepted by the parser but should not be relied on for generated cFS ABI output yet.

### Field Defaults

```syn
struct Point {
    x: f64 = 0.0
}
```

Defaults parse into the AST, but generated C/Rust structs do not currently use them to create constructors, initializers, or validation metadata.

In `0.2.x`, cFS codegen rejects field defaults until concrete initializer or defaulting semantics exist.

### Optional Fields

```syn
struct Status {
    error_code?: i32
}
```

Optional markers parse into the AST, but generated ABI structs do not currently encode optionality. Avoid optional fields for generated cFS ABI payloads in `0.1.x`.

In `0.2.x`, cFS codegen rejects optional fields until a concrete ABI representation exists.

## Under Review For 0.2

These are likely areas for intentional language work after `0.1.x`.

- Symbolic command-code resolution.
- Mission-level topic ownership and assignment validation.
- Enum codegen and ABI representation.
- Optional/default semantics.
- Dynamic and bounded array representation for cFS packet/table structs.
- More explicit table metadata.
- Better generated documentation style for C headers.
- Transitive import resolution and dependency graph validation.

## 0.1 Release Checklist

Before publishing `0.1.x`, review each feature above and choose one of:

- Stable for `0.1`
- Supported with caveats
- Parsed only
- Defer to `0.2`
- Reject before publish

The release should not imply that parsed-only features are fully supported by generated cFS output.
