set shell := ["bash", "-cu"]

cfs_root := env_var_or_default("CFS_ROOT", "/tmp/cFS")
cfs_core_inc := cfs_root + "/cfe/modules/core_api/fsw/inc"
cfs_build := cfs_root + "/build/native/default_cpu1"
bindgen_args := "-I" + cfs_root + "/osal/src/os/inc -I" + cfs_root + "/build/inc -I" + cfs_build + "/inc -I" + cfs_build + "/osal/inc -I" + cfs_root + "/psp/fsw/inc -I" + cfs_build + "/psp/inc -I" + cfs_root + "/cfe/modules/es/fsw/inc -I" + cfs_root + "/cfe/modules/evs/fsw/inc -I" + cfs_root + "/cfe/modules/fs/fsw/inc -I" + cfs_root + "/cfe/modules/sb/fsw/inc -I" + cfs_root + "/cfe/modules/tbl/fsw/inc -I" + cfs_root + "/cfe/modules/time/fsw/inc -I" + cfs_root + "/cfe/modules/msg/fsw/inc -I" + cfs_root + "/cfe/modules/resourceid/fsw/inc"
release_packages := "-p cfs-synapse-parser -p cfs-synapse-codegen-cfs -p cfs-synapse"

default:
    just --list

fmt:
    cargo fmt --all

check:
    cargo check -p cfs-synapse-parser -p cfs-synapse-codegen-cfs -p cfs-synapse -p synapse-integration-tests

check-cfs:
    CFS_DIR="{{cfs_core_inc}}" BINDGEN_EXTRA_CLANG_ARGS="{{bindgen_args}}" cargo check --workspace

test:
    cargo test -p cfs-synapse-parser -p cfs-synapse-codegen-cfs -p cfs-synapse -p synapse-integration-tests

test-release:
    cargo test -p cfs-synapse-parser -p cfs-synapse-codegen-cfs -p cfs-synapse -p synapse-integration-tests
    cargo run -p synapse-rust-canary

test-cfs:
    CFS_DIR="{{cfs_core_inc}}" BINDGEN_EXTRA_CLANG_ARGS="{{bindgen_args}}" cargo test

crap:
    cargo crap

crap-badge:
    mkdir -p target/crap docs/crap
    cargo crap --format json --output target/crap/report.json
    python3 scripts/cargo_crap_badge.py target/crap/report.json docs/crap/badge.json
    cat docs/crap/badge.json

build:
    cargo build -p cfs-synapse-parser -p cfs-synapse-codegen-cfs -p cfs-synapse -p synapse-integration-tests

build-cfs:
    CFS_DIR="{{cfs_core_inc}}" BINDGEN_EXTRA_CLANG_ARGS="{{bindgen_args}}" cargo build --workspace

gen-geometry:
    cargo run -p cfs-synapse -- --lang c -o generated synapse-integration-tests/syn/geometry_msgs.syn
    cargo run -p cfs-synapse -- --lang rust -o generated synapse-integration-tests/syn/geometry_msgs.syn

release-version-check version:
    cargo search cfs-synapse-parser --limit 1
    cargo search cfs-synapse-codegen-cfs --limit 1
    cargo search cfs-synapse --limit 1
    @echo "Check that version {{version}} does not already exist on crates.io for all three crates."

release-dry version: (release-version-check version)
    cargo release {{release_packages}} {{version}} --no-publish

release-execute version:
    cargo release {{release_packages}} {{version}} --no-publish --execute

vscode-syntax:
    code --extensionDevelopmentPath="$PWD/vscode-synapse" "$PWD"

cfs-clone:
    test -d "{{cfs_root}}/.git" || git clone https://github.com/nasa/cFS.git "{{cfs_root}}"
    git -C "{{cfs_root}}" submodule update --init --recursive

cfs-prep:
    cp "{{cfs_root}}/cfe/cmake/Makefile.sample" "{{cfs_root}}/Makefile"
    rm -rf "{{cfs_root}}/sample_defs"
    cp -r "{{cfs_root}}/cfe/cmake/sample_defs" "{{cfs_root}}/sample_defs"
    make -C "{{cfs_root}}" SIMULATION=native prep

cfs-install:
    make -C "{{cfs_root}}" install

cfs-bootstrap: cfs-clone cfs-prep cfs-install

clean:
    cargo clean
