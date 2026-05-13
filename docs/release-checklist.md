# Release Checklist

Use this checklist before tagging a Synapse release.

## 1. Confirm Scope

- Review the diff for release-facing changes:

  ```bash
  git status --short
  git log --oneline origin/main..HEAD
  ```

- Confirm README, `docs/roadmap-0.2.md`, and examples describe the current behavior.
- Confirm the release version matches the intended tag.

## 2. Update Versions

The preferred release flow uses `cargo-release` through the `justfile`:

```bash
just release-dry X.Y.Z
just release-execute X.Y.Z
```

`release-dry` checks the current crates.io versions, previews the version bump, and performs no side effects. `release-execute` updates versions, commits the release, creates the `vX.Y.Z` tag, and pushes it. The tag push triggers the GitHub release workflow.

The repo configures cargo-release with `publish = false`, so crates are not published from your local machine. GitHub Actions handles crates.io publishing when `PUBLISH_CRATE=true`.

The `justfile` release commands intentionally select only the publishable crates:

- `cfs-synapse-parser`
- `cfs-synapse-codegen-cfs`
- `cfs-synapse`

The workspace version is shared, so supporting workspace members that inherit `workspace.package.version` may still have their version updated, but they are not published.

If updating versions manually, update the workspace version and local crate dependency versions together:

- Root `Cargo.toml`
- `synapse-parser/Cargo.toml`
- `synapse-codegen-cfs/Cargo.toml`
- `synapse/Cargo.toml`

Then refresh the lockfile:

```bash
cargo check -p cfs-synapse-parser -p cfs-synapse-codegen-cfs -p cfs-synapse
```

## 3. Run Tests

Run the core suite:

```bash
cargo test \
  -p cfs-synapse-parser \
  -p cfs-synapse-codegen-cfs \
  -p cfs-synapse \
  -p synapse-integration-tests
```

Run the Rust canary:

```bash
cargo run -p synapse-rust-canary
```

Optionally run the C++ canary:

```bash
cmake -S examples/cpp-canary -B /tmp/synapse-cpp-canary-build
cmake --build /tmp/synapse-cpp-canary-build
/tmp/synapse-cpp-canary-build/synapse_cpp_canary
```

The workspace intentionally excludes `cfs-sys` from normal release checks because it needs a local cFS tree and generated cFS build headers.

## 4. Check CLI Outputs

Build and smoke-test the release binary:

```bash
cargo build --release -p cfs-synapse --bin synapse
./target/release/synapse --help
./target/release/synapse check \
  examples/mission-demo/syn/nav_app.syn \
  examples/mission-demo/syn/camera_app.syn \
  examples/mission-demo/syn/payload_app.syn
./target/release/synapse doc -o /tmp/synapse-docs synapse-integration-tests/syn/camera_msgs.syn
./target/release/synapse registry --format json synapse-integration-tests/syn/camera_msgs.syn
./target/release/synapse --lang c -o /tmp/synapse-generated synapse-integration-tests/syn/geometry_msgs.syn
./target/release/synapse --lang rust -o /tmp/synapse-generated synapse-integration-tests/syn/geometry_msgs.syn
```

## 5. Package Dry Runs

Package crates in dependency order:

```bash
cargo package -p cfs-synapse-parser
cargo package -p cfs-synapse-codegen-cfs
cargo package -p cfs-synapse
```

For crates.io dry runs:

```bash
cargo publish -p cfs-synapse-parser --dry-run
cargo publish -p cfs-synapse-codegen-cfs --dry-run
cargo publish -p cfs-synapse --dry-run
```

For the first release of a dependency chain, later dry runs can fail until the earlier crates exist on crates.io. After the support crates are published once, all dry runs should work.

## 6. Confirm GitHub Settings

For binary-only GitHub releases:

- No extra secrets are required; the release workflow uses `GITHUB_TOKEN`.

For crates.io publishing:

- Repository variable: `PUBLISH_CRATE=true`
- Repository secret: `CARGO_REGISTRY_TOKEN`

For coverage publishing:

- GitHub Pages source: GitHub Actions
- Coverage workflow publishes `docs/` plus `coverage/`
- README coverage badge reads from `coverage/badge.json`

## 7. Tag And Publish

The preferred command is:

```bash
just release-execute X.Y.Z
```

If tagging manually, create and push a semver tag:

```bash
git tag vX.Y.Z
git push origin vX.Y.Z
```

The release workflow builds archives for Linux, macOS, and Windows. If `PUBLISH_CRATE=true`, it also publishes crates in dependency order:

1. `cfs-synapse-parser`
2. `cfs-synapse-codegen-cfs`
3. `cfs-synapse`

## 8. Post-Release Checks

After the workflows finish:

- Confirm the GitHub Release has all three archives:
  - `synapse-linux-x86_64.tar.gz`
  - `synapse-macos-x86_64.tar.gz`
  - `synapse-windows-x86_64.zip`
- Confirm crates.io shows the new versions.
- Confirm `cargo install cfs-synapse` installs the expected version.
- Confirm docs.rs builds for `cfs-synapse`.
- Confirm the coverage badge and report are live on GitHub Pages.
- Add a short release note summarizing user-facing changes.
