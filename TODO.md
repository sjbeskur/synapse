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
- [x] Require `@cc(...)` for `command`.
- [x] Reject duplicate telemetry literal MIDs and command literal MID/CC pairs.
- [x] Validate literal MID command/telemetry bit patterns.
- [x] Emit command-code constants for commands.
- [x] Enum ABI representation for C and Rust.
- [x] Document bounded string storage and null-termination semantics.
- [x] Validate direct imports and qualified type references for path-based generation.
- [x] Transitive import resolution and dependency graph validation.
- [x] Imported/scoped constant resolution for attributes, e.g. `@mid(nav_app::NAV_TLM_MID)`.
- [x] Add deterministic generated file headers.

## Rough Edges

### Codegen output quality

- [x] Emit parsed `///` doc comments into generated C and Rust output.
- [ ] Decide whether generated C docs should stay as `///` or switch to Doxygen block comments.
- [ ] Add schema hash or generation timestamp to the file banner so stale headers are detectable.
- [x] Normalize hex formatting for typed integer constants and packet MID constants.

### CFE_Span_t for arrays

- [ ] `CFE_Span_t` is emitted for dynamic/bounded non-string arrays as a workaround. Validate that the target cFS tree provides this type, or document the requirement explicitly. Longer-term: design a proper ABI representation.

### Grammar vs codegen surface mismatch

- [ ] The grammar accepts optional fields, field defaults, and dynamic arrays that codegen immediately rejects. Consider either gating the grammar to match what codegen supports, or surfacing a clearer "not yet implemented" distinction in parser-level docs/errors.

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
