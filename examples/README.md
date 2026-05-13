# Synapse Examples

This directory contains small canary projects and mission-shaped examples that show how Synapse fits into C, C++, Rust, and cFS-style validation workflows.

## Rust Canary

[`rust-canary`](rust-canary/) is a minimal Rust project that generates Rust `repr(C)` bindings from local `.syn` files during its build.

Useful commands:

```bash
cargo run -p synapse-rust-canary
```

The example keeps checked-in generated output under [`rust-canary/generated`](rust-canary/generated/) so changes to generated Rust are easy to inspect.

## C++ Canary

[`cpp-canary`](cpp-canary/) is a minimal CMake project that consumes generated cFS-style C headers from `.syn` files.

Useful commands:

```bash
cmake -S examples/cpp-canary -B /tmp/synapse-cpp-canary-build
cmake --build /tmp/synapse-cpp-canary-build
/tmp/synapse-cpp-canary-build/synapse_cpp_canary
```

The example includes a tiny local [`cfe.h`](cpp-canary/include/cfe.h) shim so the generated headers can compile without a full cFS checkout.

## Mission Demo

[`mission-demo`](mission-demo/) shows multi-root mission validation across several app-owned `.syn` files.

Useful command:

```bash
synapse check \
  examples/mission-demo/syn/nav_app.syn \
  examples/mission-demo/syn/camera_app.syn \
  examples/mission-demo/syn/payload_app.syn
```

It also includes conflict fixtures under [`mission-demo/conflicts`](mission-demo/conflicts/) for duplicate telemetry MID and duplicate command MID/CC examples.

## Proposed Ground Loop Canary

A future `ground-loop` example is tracked in [`../docs/roadmap-0.2.md`](../docs/roadmap-0.2.md). The goal is to demonstrate an inspectable path from a ground-side command sender, through UDP and a CI-style ingest path, onto the cFE Software Bus, into a tiny cFS app, and back out through telemetry.

That example should stay a canary/demo rather than becoming Synapse core runtime behavior.

## Integration Message Sets

Broader `.syn` language examples live under [`../synapse-integration-tests/syn`](../synapse-integration-tests/syn/):

- [`geometry_msgs.syn`](../synapse-integration-tests/syn/geometry_msgs.syn): ROS-like geometry packets.
- [`std_msgs.syn`](../synapse-integration-tests/syn/std_msgs.syn): shared standard metadata.
- [`camera_msgs.syn`](../synapse-integration-tests/syn/camera_msgs.syn): commands, telemetry, represented enums, doc comments, tables, fixed arrays, and bounded strings.
- [`cfs_patterns.syn`](../synapse-integration-tests/syn/cfs_patterns.syn): minimal command, telemetry, and table patterns.
- [`postcard.syn`](../synapse-integration-tests/syn/postcard.syn): import and nested type examples.

See [`../docs/examples.md`](../docs/examples.md) for a longer index of sample `.syn` files and checked-in generated output.
