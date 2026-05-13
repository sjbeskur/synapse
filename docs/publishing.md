# Publishing Synapse

This document describes the release process for Synapse. The user-facing crates.io package is `cfs-synapse`, which installs the `synapse` binary. The repo also publishes two support crates:

- `cfs-synapse-parser`
- `cfs-synapse-codegen-cfs`
- `cfs-synapse`

For the short step-by-step release flow, see [`release-checklist.md`](release-checklist.md).

## Release Outputs

A release has two distribution channels:

- GitHub Release archive for users who want a prebuilt Linux `synapse` binary.
- crates.io packages for Rust users who want `cargo install cfs-synapse` or a `build.rs` dependency.

The GitHub release workflow is triggered by pushing a tag like `v0.1.0`.

## Required GitHub Settings

To publish GitHub binary archives only, no extra secrets are required. The workflow uses the standard `GITHUB_TOKEN` with `contents: write`.

To also publish to crates.io, configure:

- Repository variable: `PUBLISH_CRATE=true`
- Repository secret: `CARGO_REGISTRY_TOKEN`

If `PUBLISH_CRATE` is not set to `true`, the release workflow still builds and uploads binary archives but skips crates.io publishing.

## Local Preflight

Run the core test suite:

```bash
cargo test -p cfs-synapse-parser -p cfs-synapse-codegen-cfs -p cfs-synapse -p synapse-integration-tests
```

Build the release binary:

```bash
cargo build --release -p cfs-synapse --bin synapse
./target/release/synapse --help
./target/release/synapse check synapse-integration-tests/syn/geometry_msgs.syn
./target/release/synapse --lang c -o /tmp/synapse-check synapse-integration-tests/syn/geometry_msgs.syn
./target/release/synapse --lang rust -o /tmp/synapse-check synapse-integration-tests/syn/geometry_msgs.syn
```

Package the leaf crate:

```bash
cargo package -p cfs-synapse-parser --allow-dirty
```

`cargo package` creates the `.crate` archive locally and catches common manifest, include/exclude, and metadata problems.

## Crates.io Dry Runs

For the first release, only the parser crate can fully dry-run before anything is published:

```bash
cargo publish -p cfs-synapse-parser --dry-run
```

The other crates depend on packages that do not exist in the crates.io index until the earlier crates are published:

- `cfs-synapse-codegen-cfs` depends on `cfs-synapse-parser`.
- `cfs-synapse` depends on `cfs-synapse-parser` and `cfs-synapse-codegen-cfs`.

Before the first publish, dry-running those later crates may fail with a "no matching package named ..." error. That is expected. After the support crates exist on crates.io, all three dry runs should work.

## Manual Crates.io Publish Order

If publishing manually, publish in dependency order:

```bash
cargo publish -p cfs-synapse-parser
cargo publish -p cfs-synapse-codegen-cfs
cargo publish -p cfs-synapse
```

Wait for the crates.io index to update between steps. If a later crate cannot find the crate published immediately before it, wait a minute and retry.

## Automated Release

Create and push a version tag:

```bash
git tag v0.1.0
git push origin v0.1.0
```

The release workflow will:

1. Build the `synapse` release binary on Linux.
2. Package the binary as a release archive.
3. Attach that archive to the GitHub Release.
4. If `PUBLISH_CRATE=true`, publish crates to crates.io in dependency order.

The release archives are named:

- `synapse-linux-x86_64.tar.gz`

## Version Checklist

Before tagging a release:

1. Update crate versions together:
   - `synapse-parser/Cargo.toml`
   - `synapse-codegen-cfs/Cargo.toml`
   - `synapse/Cargo.toml`
2. Update dependency versions between local crates.
3. Update `Cargo.lock`.
4. Run the local preflight commands above.
5. Confirm the README install examples still match the release artifact names.

## Notes

The workspace intentionally excludes `cfs-sys` from the normal release test path because it requires a local cFS checkout and generated cFS build headers. Use `just cfs-bootstrap` and `just test-cfs` when you want to verify the cFS bindgen path locally.
