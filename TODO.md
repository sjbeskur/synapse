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
- [x] Remove schema-level `@mid(...)`; route logical topics through a mission manifest.
- [x] Require `@cc(...)` for `command`.
- [x] Reject duplicate telemetry topics and command topic/CC pairs.
- [x] Emit command-code constants for commands.
- [x] Enum ABI representation for C and Rust.
- [x] Document bounded string storage and null-termination semantics.
- [x] Validate direct imports and qualified type references for path-based generation.
- [x] Transitive import resolution and dependency graph validation.
- [x] Imported/scoped constant resolution for `@cc(...)`.
- [x] Add validation-only `synapse check` CLI and library API.
- [x] Add deterministic generated file headers.

## Rough Edges

### Codegen output quality

- [x] Emit parsed `///` doc comments into generated C and Rust output.
- [x] Keep generated C docs as `///` for the `0.2.x` line.
- [x] Add the Synapse package version to the generated file banner so stale headers are easier to spot.
- [x] Normalize hex formatting for typed integer constants.
- [x] Include namespace ownership in generated C enum variant macros.

### CFE_Span_t for arrays

- [ ] `CFE_Span_t` is emitted for dynamic/bounded non-string arrays as a workaround. Validate that the target cFS tree provides this type, or document the requirement explicitly. Longer-term: design a proper ABI representation.

### Grammar vs codegen surface mismatch

- [ ] The grammar accepts optional fields, field defaults, and dynamic arrays that codegen immediately rejects. Consider either gating the grammar to match what codegen supports, or surfacing a clearer "not yet implemented" distinction in parser-level docs/errors.

## Mission Routing

- [x] Keep deployment IDs out of reusable `.syn` schemas.
- [x] Validate logical command and telemetry topics against a versioned mission manifest.
- [x] Generate cFE topic-ID and MsgId mapping macros.
- [ ] Consider optional manifest policies for app/processor topic-ID partitions.
- [ ] Add reserved/system topic-ID range checks when mission requirements are defined.

## cFS Integration Gotchas

Validation and UX gaps to track while expanding cFS support:

- [x] Enforce the MsgId abstraction boundary by rejecting schema-owned MIDs.
- [x] Validate command-code uniqueness per logical command topic.
- [ ] Add mission-level checks for reserved/system topic-ID ranges.
- [ ] Add mission-level ownership checks for app/processor topic-ID partitions.
- [ ] Improve ABI checks for generated C/Rust packet structs (layout/size/offset compatibility).
- [ ] Explicitly validate string storage/null-termination policy in cFS packet/table codegen.
- [ ] Keep array representation constraints explicit (dynamic/bounded/fixed) with actionable diagnostics.
