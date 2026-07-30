# Synapse 0.3 Roadmap

Synapse 0.3 makes cFS message routing mission-owned while keeping the project a
small, easy-to-use IDL and code-generation utility.

## Release Scope

- Target standard, non-EDS cFS builds.
- Generate C packet types as the primary flight-integration output.
- Generate Rust `#[repr(C)]` packet types for ABI use only.
- Keep cFE runtime APIs, application lifecycle, Software Bus wrappers, and a
  full Rust cFS application framework out of scope.
- Keep final MsgId construction in cFE mission/platform configuration.

## Completed Milestones

1. Added logical command and telemetry topics.
2. Added human-owned `mission.toml` topic assignments and routing-header
   generation.
3. Removed schema-owned `@mid(...)`, top-level commands, legacy MsgId layout
   policies, and schema-generated MID constants.
4. Aligned routing generation with
   `cfe_core_api_msgid_mapping.h`, documented the 0.3 support contract, and
   hardened release metadata and ABI validation.

## Final Release Gate

Before tagging 0.3.0:

- Compile generated C packet and routing headers inside a current cFS checkout.
- Run a small cFS application that subscribes to and dispatches a generated
  command.
- Publish generated telemetry through the Software Bus and observe it through
  the standard lab/ground path.
- Add that real cFS integration flow to CI.

## Later Work

EDS generation or integration is a candidate for `0.4.x`. It should be treated
as a separate feature because EDS-enabled builds generate and own component
interface headers.
