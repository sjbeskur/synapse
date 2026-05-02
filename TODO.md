# TODO

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
