# Synapse One-Pager

## TL;DR

Synapse is a message-definition and code-generation utility for NASA cFS missions. It lets teams describe commands, telemetry, tables, constants, enums, and shared structs once in a small `.syn` IDL, then generate language bindings for mission software.

The bigger value is mission-wide validation: Synapse can check multiple app message definitions together and catch Software Bus conflicts, such as duplicate telemetry MIDs or duplicate command MID/CC pairs, before those conflicts reach integration.

## Why It Matters

cFS missions are built from many apps that communicate through Software Bus messages. Those messages are mission contracts. When the contracts are spread across handwritten headers, duplicated constants, and language-specific glue, integration risk grows quietly:

- Two apps can accidentally reuse the same telemetry MID.
- Two commands can collide on the same MID/CC pair.
- C and Rust bindings can drift from the same intended packet layout.
- Parsed-but-unsupported IDL features can create misleading ABI assumptions.
- Mission ID ownership can become tribal knowledge instead of checked policy.

Synapse makes these contracts explicit, generated, and checkable.

## What Synapse Does Today

- Parses `.syn` message definition files.
- Supports namespaces, imports, constants, represented enums, structs, tables, commands, and telemetry.
- Generates cFS-compatible C headers.
- Generates Rust `#[repr(C)]` bindings.
- Validates cFS ABI hazards before code generation.
- Resolves local and imported constants used in `@mid(...)` and `@cc(...)`.
- Checks multiple app roots together with `synapse check`.
- Detects duplicate telemetry MIDs and duplicate command MID/CC pairs across a mission-visible set.

## The Game-Changing Feature

Most generators answer:

> Can this one file generate code?

Synapse is moving toward answering:

> Can these apps safely coexist on one cFS Software Bus?

That mission-wide question is the important one. It turns Synapse from a code generator into a mission message utility.

## Example

```bash
synapse check mission/nav_app.syn mission/camera_app.syn mission/payload_app.syn
```

This validates each app root, loads its imports, resolves packet IDs, builds an internal mission registry, and reports conflicts like:

```text
duplicate telemetry MID `0x0801`
duplicate command MID/CC pair `0x1881`/`1`
```

## Why It Is Relevant

Synapse gives cFS teams a ROS 2-like message workflow without assuming a ROS runtime. The output remains plain mission software artifacts, but the source of truth becomes structured and language-agnostic.

That makes Synapse useful for:

- cFS apps written in different languages.
- Missions that need generated C headers plus Rust bindings.
- Early integration checks before app code lands together.
- Cleaner ownership of mission IDs and packet contracts.
- Safer evolution of commands and telemetry across releases.

## Why Not CSV?

CSV can work for a narrow packet registry: packet name, MID, command code, and maybe a few flat fields. It is familiar, easy to edit, and can be useful as an export format.

But CSV becomes strained when it is used as the source of truth for message definitions:

- Nested structs and reusable types are awkward to represent.
- Imports, namespaces, and ownership boundaries are implicit instead of modeled.
- Constants and aliases are hard to type-check and resolve safely.
- Enums, fixed arrays, bounded strings, and ABI details need extra conventions outside the table.
- Field documentation and generated-code comments become bolted-on metadata.
- Cross-language generation usually requires custom interpretation of loosely typed columns.
- Mission-wide validation depends on conventions that are easy to drift across files.

Synapse uses an IDL because cFS messages are structured contracts, not just rows of data. A `.syn` file can still generate tabular reports later, but the source model preserves the relationships and type information needed for safe C/Rust codegen and mission-level checks.

## Near-Term Direction

The current `0.2.x` work focuses on safety and clarity:

- Stronger validation for supported cFS ABI features.
- More examples and canaries for C, C++, Rust, and mission-level checks.
- Mission-wide registry checks.
- Machine-readable registry export for downstream databases, ICD tooling, and reports.
- Static HTML documentation generated from `.syn` files and doc comments.
- Future mission manifests for repeatable roots and MID range ownership.

The long-term goal is simple: make cFS message contracts easier to define, safer to generate, and harder to accidentally break.
