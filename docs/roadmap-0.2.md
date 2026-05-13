# Synapse 0.2 Roadmap

This roadmap tracks language and generator decisions for the `0.2.x` line. The `0.1.x` release established the initial crate/package shape and documented the first IDL surface. The goal for `0.2.x` is to make the utility safer, clearer, and more mission-aware without expanding it into a runtime or platform.

## Priorities

1. Make parsed-but-unsupported syntax hard to misuse.
2. Add validation for ABI-affecting cFS concepts.
3. Clarify IDL semantics where current generated output relies on convention.
4. Add mission-facing outputs, such as docs and registry exports, without changing parser semantics unnecessarily.
5. Add new syntax only when the ABI behavior is clear.

## Decision Tracker

Use these states:

- `Proposed`: suggested direction, not implemented.
- `Accepted`: direction chosen, implementation pending or in progress.
- `Implemented`: code and docs are updated.
- `Deferred`: intentionally left for a later release.
- `Rejected`: decided against for this language line.

## Comments And Generated Docs

Status: Implemented

Current syntax:

```syn
// Ordinary comment, ignored by generated output.

/// Documentation comment attached to the next declaration or field.
struct CameraId {
    /// Mission-defined camera name.
    name: string[<=32]
}
```

Direction:

- Use `//` as ordinary non-emitted comments.
- Use `///` as doc comments attached to the next declaration, enum variant, or field.
- Emit doc comments for generated C and Rust declarations and fields.
- Keep generated comment style simple for now: `///`.

Open questions:

- Should generated C comments eventually use Doxygen block style, such as `/** ... */`?
- Should generated documentation include declaration anchors stable enough for external ICD links?

## Parsed-Only ABI Hazards

Status: Implemented

Implemented behavior:

- Optional fields: `field?: Type` - implemented as a cFS codegen error.
- Field defaults: `field: Type = value` - implemented as a cFS codegen error.
- Enums used as field types - implemented as a cFS codegen error.

Direction:

- For cFS ABI codegen, reject constructs that parse but do not affect generated layout.
- Prefer clear CLI errors over silently emitting misleading ABI structs.

Remaining question:

- Should Synapse ever support generated default initializers outside the cFS ABI path?

## Dynamic And Bounded Arrays

Status: Implemented

Current syntax:

```syn
values: f32[]
samples: u8[<=256]
```

Direction:

- Keep fixed arrays as the recommended packet/table ABI form.
- Keep bounded strings as inline storage.
- Reject dynamic arrays in cFS codegen until an ownership/length model exists - implemented as a cFS codegen error.
- Reject non-string bounded arrays in cFS codegen until an inline storage plus length-field policy exists - implemented as a cFS codegen error.

Open questions:

- Should bounded arrays generate inline storage plus an explicit length field?
- Should `bytes[<=N]` be a special inline byte-buffer form?

## MID Validation

Status: Implemented

Direction:

- Require `@mid(...)` for `command` and `telemetry` - implemented as a cFS codegen error.
- Detect duplicate telemetry literal MIDs in a generated file - implemented as a cFS codegen error.
- Allow commands to share a literal MID when literal command codes differ.
- Validate command/telemetry MID bit patterns when the MID is a literal - implemented as a cFS codegen error.
- Resolve local integer constants used in `@mid(...)` for range and duplicate validation - implemented.
- Resolve directly imported integer constants used in `@mid(...)` for range and duplicate validation - implemented.
- Check multiple roots together through `synapse check` for mission-wide duplicate telemetry MIDs - implemented.

Remaining questions:

- Should a future mission manifest define owned MID ranges per app/namespace?
- Should packet IDs be validated against configurable mission policies beyond command/telemetry bit patterns?

## Command Codes

Status: Implemented

Possible syntax:

```syn
@mid(0x1880)
@cc(2)
command SetMode {
    mode: u8
}
```

Direction:

- Require `@cc(...)` on every `command`.
- Emit `_CC` constants beside command `_MID` constants.
- Reject `@cc(...)` on telemetry, struct, and table items.
- Reject duplicate literal command MID/CC pairs.
- Resolve local and directly imported integer constants used in `@cc(...)` for duplicate validation - implemented.
- Check multiple roots together through `synapse check` for duplicate command MID/CC pairs - implemented.

Remaining questions:

- Should command codes be grouped by app/namespace in generated output?

## Enum ABI Representation

Status: Implemented

Direction:

- Enum field ABI support requires an explicit integer representation, such as `enum u8 CameraMode`.
- Generate fixed-width C typedefs and Rust type aliases.
- Generate named constants for variants.
- Require explicit variant values and validate them against the representation range.
- Include namespace ownership in generated C enum variant macros when a namespace exists - implemented.

Options:

- Revisit native C `typedef enum` and Rust `#[repr(...)] enum` later if the stronger type identity is worth the ABI risk and extra generation rules.

## Strings

Status: Implemented

Current recommended form:

```syn
name: string[<=32]
```

Direction:

- Keep bounded strings as inline storage.
- Treat `string[<=N]` and `string[N]` as exactly `N` bytes.
- Do not guarantee null termination, text encoding, or C-string semantics.
- Document the optional project-level C-string convention: `N` includes the null terminator, length is found by scanning for the first null byte, and support-library helpers can provide Rust/C++ ergonomics without changing the ABI.
- Reject unbounded `string` fields in cFS codegen because they would require pointer-like ABI fields.

Open questions:

- Should a future `cstring[<=N]` distinguish null-terminated strings from byte arrays?
- Should Rust/C++ string helpers live in a small runtime/support crate and header, or be emitted beside generated code?

## Imports And Namespaces

Status: Implemented

Direction:

- Validate direct imports for path-based generation - implemented.
- Validate local and directly imported qualified type references - implemented.
- Reject unqualified references to imported types with a namespace-qualified suggestion - implemented.
- Load and validate transitive import graphs - implemented.
- Generate the root file plus transitive imports in dependency order through CLI `-o` or library `generate_files` - implemented.
- Resolve directly imported integer constants in attributes - implemented.
- Require a direct import for constants used in attributes; transitive-only constant references are rejected - implemented.
- Add CLI and library validation-only checks for CI/preflight workflows - implemented.

Remaining questions:

- How should output directories mirror namespace/import structure?

## Generated File Headers

Status: Implemented

Direction:

- Add a deterministic generated-file header comment.
- Avoid timestamps and source paths so generated output stays reproducible.

Open questions:

- Should generated headers eventually include the Synapse package version?
- Should generated C headers include include guards in addition to `#pragma once`?

## Registry And Documentation Outputs

Status: Implemented

Direction:

- Keep `.syn` files as the source of truth for message contracts.
- Use the mission registry as an exportable artifact, not as a replacement for `.syn` definitions.
- Add a machine-readable registry output for database ingestion, ICD tooling, dashboards, and other external systems - implemented for packet-level JSON and CSV through `synapse registry`.
- Keep CSV as an export/report format for packet tables, while avoiding CSV as the primary message-definition format.
- Add generated documentation output built from namespaces, imports, packet IDs, command codes, fields, types, and doc comments - implemented as searchable single-file static HTML through `synapse doc`.
- Keep these outputs in Synapse's boundary: generate and validate message-contract artifacts; do not become the database, web service, ground system, or mission configuration platform.

Implemented CLI shape:

```bash
synapse registry root_a.syn root_b.syn --format json
synapse registry root_a.syn root_b.syn --format csv -o packets.csv
synapse doc root_a.syn root_b.syn -o site/
```

Open questions:

- Should generated HTML eventually support multi-page output in addition to the single-file `index.html`?
- Should JSON include fully resolved numeric values only, or both resolved values and original symbolic expressions?
- Should later registry versions include all imported packets, only explicit roots, or both with ownership metadata?
- What schema stability promise should registry JSON make across minor releases?

## CLI And Release Polish

Status: Implemented

Direction:

- Keep `synapse` usable as a direct generation command for the common case.
- Keep explicit subcommands for `generate`, `check`, `doc`, and `registry`.
- Provide useful `--help` examples for core workflows.
- Publish coverage HTML and a coverage badge through GitHub Pages without requiring a third-party coverage service.

Remaining questions:

- Should `synapse --version` include enabled codegen backends or only the package version?
- Should release artifacts include generated shell completions?

## Ground Loop Canary

Status: Proposed

Direction:

- Add an example that demonstrates a transparent ground-command loop into a cFS app.
- Keep it under `examples/` as a canary/demo, not as Synapse core runtime behavior.
- Reuse Synapse-generated contracts on both sides where practical:
  - generated C headers for the cFS test app
  - generated Rust bindings or registry output for the ground sender
- Make every boundary inspectable: packed command bytes, UDP datagram, ingest app receive, Software Bus submit, target app command receive, telemetry response, and ground-side decode.

Possible shape:

```text
examples/ground-loop/
  syn/
    mission_ids.syn
    demo_msgs.syn
  ground-sender/
    Cargo.toml
    src/main.rs
  cfs-app/
    README.md
    src/
```

The first useful slice would send one command packet over UDP to a CI-style ingest path, have a tiny cFS app receive and log the MID/CC/payload, then emit telemetry that a ground-side receiver can inspect.

Open questions:

- Should the first version target `CI_LAB`/`TO_LAB`, or a tiny custom ingest/output pair built only for the canary?
- How much cFE header packing should live in the example versus generated support helpers?
- Should this include Wireshark/tcpdump inspection notes for the UDP boundary?
