# Release Notes

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
