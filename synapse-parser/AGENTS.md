# Agent Notes

This crate parses Synapse (`.syn`) files with Pest and builds the AST used by the cFS code generator.

## Commands

From the workspace root:

```bash
cargo test -p cfs-synapse-parser
cargo run -p cfs-synapse-parser --example parse_synapse -- synapse-parser/examples/sample.syn
```

From this crate:

```bash
cargo test
cargo run --example parse_synapse -- examples/sample.syn
```

## Files

- `synapse.pest`: Pest PEG grammar.
- `src/lib.rs`: exposes the public parser crate surface.
- `src/grammar_tests.rs`: Pest grammar acceptance/rejection tests.
- `src/ast.rs`: public AST data model.
- `src/ast/builder.rs`: converts Pest pairs into the public AST.
- `src/ast/tests.rs`: AST construction tests.
- `examples/parse_synapse.rs`: parser debugging CLI.
- `examples/sample.syn`: sample language file.

## Language Shape

Top-level items are `namespace`, `import`, `const`, `enum`, `struct`, `command`, `telemetry`, `table`, and legacy `message`.

Use `struct` for plain reusable ABI-shaped data. Use `command` for cFS Software Bus command packets; generators add `CFE_MSG_CommandHeader_t`. Use `telemetry` for cFS Software Bus telemetry packets; generators add `CFE_MSG_TelemetryHeader_t`. Use `table` for cFS Table Services payload data; generators do not add a Software Bus header. Legacy `message` remains accepted as a generic Software Bus packet.

Types include Rust-style primitives (`f32`, `f64`, `i8` through `i64`, `u8` through `u64`, `bool`, `bytes`), `string`, and user-defined refs such as `Point` or `geometry::Point`.

Array suffixes are:

- `[]`: dynamic
- `[N]`: fixed-size
- `[<=N]`: bounded dynamic

For `string`, `string[<=N]` means a bounded-length string, not an array of strings.

## Pest Ordering Rules

Pest ordered choice uses the first successful branch, so keep these grammar orderings intact:

- `base_type`: `string_type` before `primitive_type` before `type_ref`.
- `literal`: `float_lit` before `hex_lit` before `int_lit` before identifiers.

These prevent keywords and partial numeric literals from being parsed as broader fallback rules.
