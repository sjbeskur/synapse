# Examples

The most useful examples live in `synapse-integration-tests/syn`. These files are parsed in integration tests and used to verify generated C/Rust output.

## Current cFS Examples

- [`std_msgs.syn`](../synapse-integration-tests/syn/std_msgs.syn): small shared `Time` and `Header` structs used by other examples.
- [`cfs_patterns.syn`](../synapse-integration-tests/syn/cfs_patterns.syn): minimal `table`, `command`, and `telemetry` definitions.
- [`geometry_msgs.syn`](../synapse-integration-tests/syn/geometry_msgs.syn): ROS-like geometry structs and stamped telemetry packets.
- [`camera_msgs.syn`](../synapse-integration-tests/syn/camera_msgs.syn): camera control examples, including commands, telemetry, nested structs, bounded strings, fixed arrays, doc comments, and a calibration table.

## Generated Output

Checked-in generated geometry output is available in:

- [`generated/geometry_msgs.h`](../generated/geometry_msgs.h)
- [`generated/geometry_msgs.rs`](../generated/geometry_msgs.rs)

Regenerate those files with:

```bash
cargo run -p cfs-synapse -- --lang c -o generated synapse-integration-tests/syn/geometry_msgs.syn
cargo run -p cfs-synapse -- --lang rust -o generated synapse-integration-tests/syn/geometry_msgs.syn
```

Or, with `just`:

```bash
just gen-geometry
```

## Parser Examples

The parser crate also has exploratory examples under `synapse-parser/examples`.

- [`sample.syn`](../synapse-parser/examples/sample.syn): broad parser syntax coverage, including some features that are parsed but not fully generated in `0.1.x`.
- [`geometry.syn`](../synapse-parser/examples/geometry.syn): older parser-oriented geometry sample.
- [`parse_synapse.rs`](../synapse-parser/examples/parse_synapse.rs): small parser inspection utility.

Prefer the integration-test examples for current cFS generator behavior.
