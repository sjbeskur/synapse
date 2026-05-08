# Synapse Rust Canary

This small standalone crate demonstrates using Synapse from `build.rs`.

It generates Rust `#[repr(C)]` bindings from `.syn` files at compile time, then constructs a command and telemetry packet using the generated types. A tiny local `cfs_sys` module stubs the cFS packet header types so the example can run without a full cFS checkout.

Run it from the repository root:

```bash
cargo run --manifest-path examples/rust-canary/Cargo.toml
```

The interesting pieces are:

- `syn/mission_ids.syn`: mission-owned MID and command-code constants.
- `syn/demo_msgs.syn`: imports those constants in `@mid(...)` and `@cc(...)`.
- `build.rs`: calls `cfs_synapse::generate_files`.
- `src/main.rs`: includes and uses the generated bindings.
