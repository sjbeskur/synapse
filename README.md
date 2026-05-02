# Synapse cFS SDK

Synapse is a small IDL and code generator for NASA cFS-friendly data types. It lets one `.syn` file describe plain ABI-compatible structs, Software Bus packets, and table payloads, then generates C headers and Rust `#[repr(C)]` bindings.

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

## UDP and Tables

When communicating with cFS through UDP apps such as `CI_LAB` or `TO_LAB`, UDP is only the transport. The bytes entering or leaving the Software Bus are still cFS packets, so generated `command` and `telemetry` types need the cFS header.

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
