# Release Notes

## Unreleased

### Added

- Added logical `commands` groups, with function codes unique within each
  command topic.
- Added MID-free telemetry topics.
- Added versioned mission TOML parsing and strict logical-topic assignment
  validation.
- Added `synapse check --manifest` for mission-aware validation.
- Added `synapse routes --manifest` for generating cFE topic-ID and MsgId
  mapping headers.
- Added mission-manifest and routing-header library APIs.

### Changed

- Updated generated routing headers for current cFE mission builds by including
  `cfe_core_api_msgid_mapping.h`.
- Packet registries and documentation now expose logical topic names and no
  longer contain legacy MID fields.
- Migrated the mission demo from schema-owned MIDs to mission-owned topic IDs.
- Removed schema-level `@mid(...)`, top-level `command` declarations,
  schema-generated MID constants, raw MsgId layout validation, and
  `--msgid-layout`.
- Commands must now be declared inside a logical `commands` group.
- Command function codes are validated against the generated `u16` ABI
  constant type.
- Defined the 0.3 support boundary: standard non-EDS cFS, C as the primary
  flight-integration output, and Rust bindings limited to ABI types.

### Compatibility

- EDS-enabled cFS builds are not supported in 0.3.
- Generated Rust does not wrap cFE runtime APIs or provide a Rust cFS
  application framework.

## v0.2.13

This release adds configurable cFE message ID layout validation for missions that do not use the legacy CCSDS-style `MISSION_MSG_V1` bit layout.

### Added

- Added `--msgid-layout <ccsds-v1|opaque>` to `check`, `generate`, `doc`, and `registry`.
- Kept `ccsds-v1` as the default, preserving the existing validation rule that command MIDs have bit `0x1000` set and telemetry MIDs have that bit clear.
- Added `opaque` mode for missions where MsgIds are mission-owned opaque values. In this mode, Synapse still resolves MIDs and checks duplicate telemetry MIDs and duplicate command MID/CC pairs, but it does not infer packet kind from raw MID bits.
- Added option-bearing library APIs such as `check_paths_with_options`, `generate_path_with_options`, `generate_docs_with_options`, and `generate_registry_with_options`.

### Documentation

- Clarified that the legacy `0x1000` command/telemetry bit check is a Synapse `ccsds-v1` policy assumption, not a universal cFS rule.
- Documented `opaque` MsgId validation in the README, language reference, registry docs, mission validation notes, and `0.2.x` roadmap.

### Compatibility

- Existing CLI behavior is unchanged when `--msgid-layout` is omitted.
- Existing library APIs remain available and use the default `ccsds-v1` policy.
