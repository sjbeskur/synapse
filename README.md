# Synapse cFS SDK

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

Tagged releases attach archives for common platforms:

- `synapse-linux-x86_64.tar.gz`
- `synapse-macos-x86_64.tar.gz`
- `synapse-windows-x86_64.zip`

Download the archive for your platform from the latest GitHub Release, extract it, and place the `synapse` executable somewhere on your `PATH`.

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
synapse --lang <c|rust> [-o <out-dir>] <file.syn>
```

- `--lang c` generates a cFS C header (`.h`) that includes `cfe.h`.
- `--lang rust` generates Rust `#[repr(C)]` bindings (`.rs`) that reference `cfs_sys` header types by default.
- Without `-o`, generated code is written to stdout.

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

The library facade also exposes `generate_rust_file`, `generate_file`, and `generate_str` for custom build flows.

## Mental Model

Use `struct` for plain data layout:

```syn
struct Point {
    x: f64
    y: f64
}
```

Plain structs do not include a cFS message header. They are useful as nested payload types and other reusable C-compatible data.

Use `command` for Software Bus packets sent to an app:

```syn
@mid(0x1880)
command SetMode {
    mode: u8
}
```

Generated commands place `CFE_MSG_CommandHeader_t` first.

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

Generated tables do not include a Software Bus header. They are plain table data. The legacy `message` keyword still works for generic Software Bus packets, but new files should prefer `command` or `telemetry`.

## Language Surface

The cFS generator currently emits:

- `namespace` prefixes for generated C type names, such as `geometry_msgs_Point_t`.
- `import` declarations as C `#include` lines or Rust `use crate::...` lines.
- `const` declarations as C `#define`s or Rust `pub const`s.
- `struct` and `table` definitions as plain data structs.
- `command`, `telemetry`, and legacy `message` definitions as Software Bus packet structs with cFS headers.
- `@mid(...)` attributes as message ID constants.
- Fixed arrays, bounded strings, dynamic arrays, and namespaced type references.

Enums, field defaults, and optional field markers are parsed into the AST, but cFS code generation for those pieces is still early. Prefer plain integer fields for generated packet payloads until enum emission and optional/default handling are completed.

## Documentation Comments

Use `##` for comments that should attach to the next declaration or field and eventually feed generated documentation:

```syn
## Stable identifier for one physical or logical camera.
struct CameraId {
    ## Mission-defined camera name.
    name: string[<=32]
}
```

Use single `#` comments for ordinary notes that should not become API documentation.

## UDP and Tables

When communicating with cFS through UDP apps such as `CI_LAB` or `TO_LAB`, UDP is only the transport. The bytes entering or leaving the Software Bus are still cFS packets, so generated `command` and `telemetry` types need the cFS header.

Table Services are different: the table data itself is normally a plain struct without a Software Bus header. Commands that load, validate, or activate tables are Software Bus command messages, but the table buffer/file is just table data.

## Examples

Sample `.syn` files live in `synapse-integration-tests/syn`.

- `geometry_msgs.syn`: ROS-like geometry telemetry packets.
- `cfs_patterns.syn`: minimal command, telemetry, and table examples.
- `camera_msgs.syn`: camera control syntax coverage, including commands to set mode/exposure, a command to send updated intrinsics, telemetry for camera status, and a `CameraCalibration` table containing persistent calibration data. It also exercises parsed enum and doc-comment support.

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

## Editor Support

A local VS Code syntax-highlighting extension lives in `vscode-synapse`.

```bash
just vscode-syntax
```

This launches VS Code with the extension development path pointed at the local Synapse grammar.
