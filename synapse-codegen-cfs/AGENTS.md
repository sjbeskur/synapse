# Agent Notes

This crate turns the parsed Synapse AST into cFS-oriented C headers and Rust
`repr(C)` bindings, and validates that the source model is safe for those ABI
targets.

## Commands

From the workspace root:

```bash
cargo test -p cfs-synapse-codegen-cfs
```

## Files

- `src/lib.rs`: small public facade and re-exports.
- `src/types.rs`: public options, packet facts, and generated-file constants.
- `src/error.rs`: `CodegenError` and its display text.
- `src/constants.rs`: local/imported integer constant resolution.
- `src/validate.rs`: cFS support checks and packet fact collection.
- `src/c.rs`: NASA cFS C header emission.
- `src/rust.rs`: Rust `repr(C)` binding emission.
- `src/util.rs`: shared formatting, naming, attribute, and literal helpers.
- `src/tests.rs`: codegen and validation tests.

## Boundaries

Keep parser behavior in `cfs-synapse-parser`. This crate should only validate
and emit from the AST it receives.
