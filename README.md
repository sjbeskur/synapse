# Synapse cFS SDK

Synapse is a small IDL and code generator for NASA cFS-friendly data types. It lets one `.syn` file describe plain ABI-compatible structs and Software Bus messages, then generates C headers and Rust `#[repr(C)]` bindings.

## Mental Model

Use `struct` for plain data layout:

```syn
struct NavConfig {
    max_speed: f64
    enabled: bool
    frame_id: string[<=32]
}
```

Plain structs do not include a cFS message header. They are useful as nested payload types, table contents, configuration blobs, and other C-compatible data.

Use `message` for packets that travel on the cFS Software Bus:

```syn
@mid(0x0801)
message NavTlm {
    position: geometry_msgs::Point
}
```

Generated messages place the cFS header first. Telemetry messages use `CFE_MSG_TelemetryHeader_t`; command messages use `CFE_MSG_CommandHeader_t`.

## UDP and Tables

When communicating with cFS through UDP apps such as `CI_LAB` or `TO_LAB`, UDP is only the transport. The bytes entering or leaving the Software Bus are still cFS messages, so generated `message` types need the cFS command or telemetry header.

Table Services are different: the table data itself is normally a plain struct without a Software Bus header. Commands that load, validate, or activate tables are Software Bus command messages, but the table buffer/file is just table data.

## Workspace

- `synapse-parser`: Pest grammar and AST builder for `.syn`
- `synapse-codegen-cfs`: C and Rust cFS code generation
- `synapse`: CLI frontend
- `cfs-sys`: bindgen wrapper for selected cFS types
- `synapse-integration-tests`: sample `.syn` files and generated-code checks
- `generated`: checked-in generated examples

## Common Commands

This repo includes a `justfile`.

```bash
just test
just gen-geometry
```

To run the full suite including `cfs-sys`, bootstrap cFS first:

```bash
just cfs-bootstrap
just test-cfs
```

By default cFS is expected at `/tmp/cFS`. Override with:

```bash
CFS_ROOT=/path/to/cFS just test-cfs
```
