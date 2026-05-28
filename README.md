# Synapse cFS SDK

[![CI](https://github.com/sjbeskur/synapse/actions/workflows/ci.yml/badge.svg)](https://github.com/sjbeskur/synapse/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/sjbeskur/synapse?sort=semver)](https://github.com/sjbeskur/synapse/releases/latest)
[![Crates.io](https://img.shields.io/crates/v/cfs-synapse.svg)](https://crates.io/crates/cfs-synapse)
[![docs.rs](https://img.shields.io/docsrs/cfs-synapse)](https://docs.rs/cfs-synapse)
[![Coverage](https://img.shields.io/endpoint?url=https%3A%2F%2Fsjbeskur.github.io%2Fsynapse%2Fcoverage%2Fbadge.json)](https://sjbeskur.github.io/synapse/coverage/)
[![CRAP](https://img.shields.io/endpoint?url=https%3A%2F%2Fsjbeskur.github.io%2Fsynapse%2Fcrap%2Fbadge.json)](https://github.com/sjbeskur/synapse/actions/workflows/coverage.yml)
[![License](https://img.shields.io/crates/l/cfs-synapse.svg)](LICENSE)

Synapse is a small interface definition language and code generator for NASA cFS-friendly data types. A `.syn` file can describe plain ABI-compatible structs, cFS Software Bus commands and telemetry packets, and cFS Table Services payloads; the `synapse` CLI turns those definitions into C headers or Rust `#[repr(C)]` bindings.

## Quick Start

Install `synapse` with the path that best fits your environment.

For C developers and CI jobs without Rust installed, download a prebuilt binary from GitHub Releases:

```bash
curl -L https://github.com/sjbeskur/synapse/releases/latest/download/synapse-linux-x86_64.tar.gz | tar -xz
./synapse --help
```

For Rust users, install the `cfs-synapse` package from crates.io. The installed executable is still named `synapse`:

```bash
cargo install cfs-synapse
synapse --help
```

Generate C and Rust bindings from a `.syn` file:

```bash
synapse --lang c -o generated synapse-integration-tests/syn/geometry_msgs.syn
synapse --lang rust -o generated synapse-integration-tests/syn/geometry_msgs.syn
```

The output file is named after the input file. For example, the commands above write `generated/geometry_msgs.h` and `generated/geometry_msgs.rs`.

## Install

### Prebuilt Binaries

Tagged releases attach a Linux archive:

- `synapse-linux-x86_64.tar.gz`

Download the archive from the latest GitHub Release, extract it, and place the `synapse` executable somewhere on your `PATH`. For other platforms, install from crates.io with Cargo.

### Cargo

```bash
cargo install cfs-synapse
```

The crates.io package is named `cfs-synapse`; the installed command is `synapse`.

### From Source

Use the source workflow when contributing to Synapse itself:

```bash
cargo test -p cfs-synapse-parser -p cfs-synapse-codegen-cfs -p cfs-synapse -p synapse-integration-tests
cargo run -p cfs-synapse -- --lang c -o generated synapse-integration-tests/syn/geometry_msgs.syn
```

If you have `just` installed, the common contributor workflow is:

```bash
just test
just gen-geometry
```

The default test workflow skips the `cfs-sys` crate because it needs a local cFS tree and generated cFS build headers. To include it, bootstrap cFS first:

```bash
just cfs-bootstrap
just test-cfs
```

By default cFS is expected at `/tmp/cFS`. Override it with:

```bash
CFS_ROOT=/path/to/cFS just test-cfs
```

## CLI

```bash
synapse check <file.syn> [more-roots.syn ...]
synapse doc [-o <out-dir>] <file.syn> [more-roots.syn ...]
synapse registry [--format <json|csv>] [-o <file>] <file.syn> [more-roots.syn ...]
synapse --lang <c|rust> [-o <out-dir>] <file.syn>
synapse generate --lang <c|rust> [-o <out-dir>] <file.syn>
```

- `check` validates input roots, their import graphs, and cFS codegen support without writing generated output. Multiple roots are checked together for mission-wide telemetry MID and command MID/CC conflicts.
- `doc` generates static HTML documentation for input roots, their import graphs, packet IDs, command codes, fields, types, and doc comments. Without `-o`, HTML is written to stdout; with `-o`, Synapse writes `index.html`.
- `registry` emits a validated packet registry for input roots as JSON or CSV. Without `-o`, registry output is written to stdout.
- `--msgid-layout <ccsds-v1|opaque>` selects MID validation policy. The default is `ccsds-v1`, which validates the legacy `0x1000` command/telemetry bit. Use `opaque` for missions where cFE treats MsgIds as mission-owned opaque values.
- `--lang c` generates a cFS C header (`.h`) that includes `cfe.h`.
- `--lang rust` generates Rust `#[repr(C)]` bindings (`.rs`) that reference `cfs_sys` header types by default.
- Without `-o`, generated code is written to stdout.
- With `-o <out-dir>`, Synapse writes the root file and its transitive imports in dependency order.
- Add `--single-file` with `-o` to write only the requested root file.

## Build Scripts

Rust projects can use Synapse from `build.rs` without requiring the `synapse` executable to be installed on `PATH`.

```toml
[build-dependencies]
cfs-synapse = "0.1"
```

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=syn/my_msgs.syn");

    let out_dir = std::env::var("OUT_DIR")?;
    cfs_synapse::generate_c_file("syn/my_msgs.syn", out_dir)?;

    Ok(())
}
```

The library facade also exposes `check_path`, `check_str`, `generate_rust_file`, `generate_file`, `generate_files`, `generate_path`, and `generate_str` for custom build flows. Path-based generation validates the import graph rooted at the input file. Use `generate_files` when a build should emit the root file plus its transitive imports.

## Mental Model

Use `struct` for plain data layout:

```syn
struct Point {
    x: f64
    y: f64
}
```

Plain structs do not include a cFS message header. They are useful as nested payload types and other reusable C-compatible data.

Use represented `enum`s for small named integer domains:

```syn
enum u8 CameraMode {
    Standby = 0
    Preview = 1
}
```

Represented enums generate fixed-width C/Rust aliases and named constants. The explicit representation is what makes them safe to use in generated cFS packet and table fields.

Use `command` for Software Bus packets sent to an app:

```syn
@mid(0x1880)
@cc(1)
command SetMode {
    mode: CameraMode
}
```

Generated commands place `CFE_MSG_CommandHeader_t` first and emit both `_MID` and `_CC` constants. Commands may share a command MID when their literal command codes differ.

Use `telemetry` for Software Bus packets published by an app:

```syn
@mid(0x0801)
telemetry NavState {
    position: geometry_msgs::Point
}
```

Generated telemetry packets place `CFE_MSG_TelemetryHeader_t` first.

Use `table` for cFS Table Services payload data:

```syn
table NavConfig {
    max_speed: f64
    enabled: bool
    frame_id: string[<=32]
}
```

Generated tables do not include a Software Bus header. They are plain table data. The legacy `message` keyword still parses for older files, but cFS codegen rejects it. Use explicit `command` or `telemetry` packets.

## Language Surface

The cFS generator currently emits:

- `namespace` prefixes for generated C type names, such as `geometry_msgs_Point_t`.
- `import` declarations as C `#include` lines or Rust `use crate::...` lines.
- `const` declarations as C `#define`s or Rust `pub const`s.
- Represented enums such as `enum u8 CameraMode` as fixed-width type aliases and constants.
- `struct` and `table` definitions as plain data structs.
- `command` and `telemetry` definitions as Software Bus packet structs with cFS headers.
- Required `@mid(...)` attributes as message ID constants.
- Required command `@cc(...)` attributes as command-code constants.
- Fixed arrays, bounded strings, and namespaced type references.

Enums need an explicit integer representation when used in generated cFS fields, for example `enum u8 CameraMode`. Unrepresented enums, unbounded strings, optional field markers, field defaults, dynamic arrays, and non-string bounded arrays are parsed but rejected by cFS codegen until concrete ABI and initializer semantics exist. Prefer represented enums, fixed arrays, and bounded strings for generated packet payloads.

See `docs/language.md` for the current language status and `0.1.x` review checklist.
See `docs/types.md` for the supported type forms and generated C/Rust mappings.
See `docs/one-pager.md` for a short explanation of why Synapse is relevant.
See `docs/mission.md` for the mission-wide validation and registry concept.
See `docs/registry.md` for the JSON and CSV packet registry schema.
See `docs/roadmap-0.2.md` for the `0.2.x` language, validation, and output status.

## Documentation Comments

Use `///` for comments that should attach to the next declaration or field and feed generated code comments and HTML documentation:

```syn
/// Stable identifier for one physical or logical camera.
struct CameraId {
    /// Mission-defined camera name.
    name: string[<=32]
}
```

Use `//` comments for ordinary notes that should not become API documentation.

## UDP and Tables

When communicating with cFS through UDP apps such as `CI_LAB` or `TO_LAB`, UDP is only the transport. The bytes entering or leaving the Software Bus are still cFS packets, so generated `command` and `telemetry` types need the cFS header.

Table Services are different: the table data itself is normally a plain struct without a Software Bus header. Commands that load, validate, or activate tables are Software Bus command messages, but the table buffer/file is just table data.

## Examples

Sample `.syn` files live in `synapse-integration-tests/syn`.

- `geometry_msgs.syn`: ROS-like geometry telemetry packets.
- `cfs_patterns.syn`: minimal command, telemetry, and table examples.
- `camera_msgs.syn`: camera control syntax coverage, including a represented camera mode enum, commands to set mode/exposure, a command to send updated intrinsics, telemetry for camera status, and a `CameraCalibration` table containing persistent calibration data. It also exercises doc-comment support.

See `examples/README.md` for runnable canary projects and the mission demo.
See `docs/examples.md` for links to all sample `.syn` files and checked-in generated output.

## Workspace

- `cfs-synapse-parser`: Pest grammar and AST builder for `.syn`
- `cfs-synapse-codegen-cfs`: C and Rust cFS code generation
- `cfs-synapse`: public library facade and `synapse` CLI binary
- `cfs-sys`: bindgen wrapper for selected cFS types
- `synapse-integration-tests`: sample `.syn` files and generated-code checks
- `generated`: checked-in generated geometry examples

## Release Model

CI runs the core test suite on pull requests and pushes to `main`. Pushing a tag like `v0.1.0` runs the release workflow, builds release binaries, and attaches platform archives to the GitHub Release.

Crates.io publishing is gated behind the repository variable `PUBLISH_CRATE=true` and the `CARGO_REGISTRY_TOKEN` secret. When enabled, the release workflow publishes `cfs-synapse-parser`, `cfs-synapse-codegen-cfs`, and then `cfs-synapse`.

See `docs/release-checklist.md` for the step-by-step release checklist.
See `docs/publishing.md` for publishing workflow details and first-publish notes.
See `docs/release-notes.md` for release notes.

## Editor Support

A local VS Code syntax-highlighting extension lives in `vscode-synapse`.

```bash
just vscode-syntax
```

This launches VS Code with the extension development path pointed at the local Synapse grammar.
