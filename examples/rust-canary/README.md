# Synapse Rust Canary

This small standalone crate demonstrates using Synapse from `build.rs`.

It generates Rust `#[repr(C)]` bindings from `.syn` files at compile time into `generated/`, then constructs a command and telemetry packet using the generated types. A tiny local `cfs_sys` module stubs the cFS packet header types so the example can run without a full cFS checkout.

This canary verifies message-type generation and ABI shape only. It does not
wrap or exercise cFE runtime APIs.

Run it from the repository root:

```bash
cargo run -p synapse-rust-canary
```

The interesting pieces are:

- `syn/demo_msgs.syn`: declares a logical command group and telemetry topic.
- Command codes remain schema-owned through `@cc(...)`; the mission manifest
  owns the runtime cFS message IDs.
- `build.rs`: calls `cfs_synapse::generate_files` and writes `generated/*.rs`.
- `src/main.rs`: includes and uses the generated bindings.
