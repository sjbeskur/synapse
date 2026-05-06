# TODO

## v0.2.x Language Review

See `docs/roadmap-0.2.md` for the decision tracker.

- [x] Comment syntax and generated documentation style.
- [x] Reject enum fields in cFS codegen until an ABI representation exists.
- [x] Reject optional fields in cFS codegen until an ABI representation exists.
- [x] Reject field defaults in cFS codegen until initializer semantics exist.
- [x] Reject dynamic arrays in cFS codegen until an ownership/length model exists.
- [x] Reject non-string bounded arrays in cFS codegen until an inline representation exists.
- [x] Reject legacy `message` in cFS codegen; use `command` or `telemetry`.
- [x] Require `@mid(...)` for `command` and `telemetry`.
- [x] Reject duplicate literal MIDs in one generated file.
- [ ] MID validation: command/telemetry ranges.
- [ ] Command metadata, including possible `@cc(...)` command code support.
- [ ] Enum ABI representation for C and Rust.
- [x] Document bounded string storage and null-termination semantics.
- [ ] Import and namespace resolution across multiple files.
- [ ] Generated file headers and documentation style.

## Explore MID Ownership and Validation

Message IDs need a clearer ownership model before the DSL grows much further.

Ideas to explore:

- Namespace-scoped MID constants, e.g. `nav_app::SET_MODE_CMD_MID`.
- Codegen names that preserve namespace ownership:
  - C: `NAV_APP_SET_MODE_CMD_MID`
  - Rust: module-scoped `SET_MODE_CMD_MID`
- Validation for missing MIDs on `command` and `telemetry`.
- Validation for duplicate MIDs, probably globally for a generated mission bundle.
- Validation that command MIDs and telemetry MIDs land in the expected cFS ranges/bit patterns.
- Optional namespace-level reserved ranges, for example command and telemetry ranges per app.
- Constant resolution for `@mid(SET_MODE_CMD_MID)` and `@mid(nav_app::SET_MODE_CMD_MID)`.

Open design question: keep using `const` for MID declarations, or add a dedicated MID block once the desired workflow is clearer.
