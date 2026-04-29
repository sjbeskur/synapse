use std::fs;
use std::path::Path;
use std::process::Command;
use synapse_codegen_cfs::RustOptions;
use synapse_parser::ast::parse;

fn syn_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("syn")
}

fn read_and_parse(name: &str) -> synapse_parser::ast::SynFile {
    let src = fs::read_to_string(syn_dir().join(name))
        .unwrap_or_else(|e| panic!("read {name}: {e}"));
    parse(&src).unwrap_or_else(|e| panic!("parse {name}:\n{e}"))
}

// ── Parse ──────────────────────────────────────────────────────────────────────

#[test]
fn std_msgs_parses() {
    let f = read_and_parse("std_msgs.syn");
    // namespace + Time + Header
    assert_eq!(f.items.len(), 3);
}

#[test]
fn geometry_msgs_parses() {
    let f = read_and_parse("geometry_msgs.syn");
    // namespace + import + 29 types
    assert_eq!(f.items.len(), 31);
}

// ── cFS C codegen ──────────────────────────────────────────────────────────────

#[test]
fn cfs_c_codegen_geometry_msgs() {
    let out = synapse_codegen_cfs::generate_c(&read_and_parse("geometry_msgs.syn"));

    assert!(out.contains("#pragma once"));
    assert!(out.contains("#include \"cfe.h\""));
    assert!(out.contains("#include \"std_msgs.h\""));

    // All 14 stamped messages get MID defines
    for (name, mid) in STAMPED_MIDS {
        let screaming = to_screaming_snake(name);
        assert!(
            out.contains(&format!("#define {}_MID  {}", screaming, mid)),
            "missing C MID define for {name}"
        );
    }

    // All stamped messages are telemetry (MIDs 0x0800–0x080D, bit 12 clear)
    assert!(out.contains("CFE_MSG_TelemetryHeader_t Header;"));
    assert!(!out.contains("CFE_MSG_CommandHeader_t Header;"));

    // Plain structs get generated without cFS headers
    assert!(out.contains("} geometry_msgs_Vector3_t;"));
    assert!(out.contains("} geometry_msgs_Pose_t;"));

    // Spot-check stamped message structs
    assert!(out.contains("} geometry_msgs_AccelStamped_t;"));
    assert!(out.contains("} geometry_msgs_TransformStamped_t;"));
    assert!(out.contains("} geometry_msgs_WrenchStamped_t;"));

    // Covariance fixed array
    assert!(out.contains("    double covariance[36];"));

    // Cross-namespace field (plain C, no namespace qualifier)
    assert!(out.contains("    std_msgs_Header_t header;"));
}

#[test]
fn cfs_c_codegen_std_msgs() {
    let out = synapse_codegen_cfs::generate_c(&read_and_parse("std_msgs.syn"));
    assert!(out.contains("} std_msgs_Time_t;"));
    assert!(out.contains("} std_msgs_Header_t;"));
    assert!(out.contains("    uint32_t sec;"));
    assert!(out.contains("    uint32_t seq;"));
}

#[test]
fn generated_c_geometry_msgs_compiles() {
    let std_msgs = synapse_codegen_cfs::generate_c(&read_and_parse("std_msgs.syn"));
    let geometry_msgs = synapse_codegen_cfs::generate_c(&read_and_parse("geometry_msgs.syn"));

    let dir = std::env::temp_dir().join(format!("synapse-cc-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("create temp cc dir");
    fs::write(
        dir.join("cfe.h"),
        r#"
#pragma once
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef struct {
    uint8_t bytes[16];
} CFE_MSG_TelemetryHeader_t;

typedef struct {
    uint8_t bytes[16];
} CFE_MSG_CommandHeader_t;

typedef struct {
    const void *Data;
    size_t Size;
} CFE_Span_t;
"#,
    )
    .expect("write cfe.h stub");
    fs::write(dir.join("std_msgs.h"), std_msgs).expect("write std_msgs.h");
    fs::write(dir.join("geometry_msgs.h"), geometry_msgs).expect("write geometry_msgs.h");
    fs::write(
        dir.join("check.c"),
        r#"
#include "geometry_msgs.h"
"#,
    )
    .expect("write C check source");

    let output = Command::new("cc")
        .arg("-std=c99")
        .arg("-fsyntax-only")
        .arg("-I")
        .arg(&dir)
        .arg(dir.join("check.c"))
        .output()
        .expect("run cc");

    assert!(
        output.status.success(),
        "generated C did not compile\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

// ── cFS Rust codegen ───────────────────────────────────────────────────────────

#[test]
fn cfs_rust_codegen_geometry_msgs() {
    let opts = RustOptions::default();
    let out = synapse_codegen_cfs::generate_rust(&read_and_parse("geometry_msgs.syn"), &opts);

    // All 14 stamped messages get MID consts
    for (name, mid) in STAMPED_HEX {
        let screaming = to_screaming_snake(name);
        assert!(
            out.contains(&format!("pub const {}_MID: u16 = {};", screaming, mid)),
            "missing Rust MID const for {name}"
        );
    }

    // All are telemetry
    assert!(out.contains("pub cfs_header: cfs_sys::CFE_MSG_TelemetryHeader_t,"));
    assert!(!out.contains("pub cfs_header: cfs_sys::CFE_MSG_CommandHeader_t,"));

    // repr(C) on every struct
    assert!(out.contains("#[repr(C)]"));

    // Spot-check
    assert!(out.contains("pub struct PoseStamped {"));
    assert!(out.contains("pub struct TransformStamped {"));
    assert!(out.contains("pub covariance: [f64; 36],"));
}

#[test]
fn cfs_rust_codegen_std_msgs() {
    let opts = RustOptions::default();
    let out = synapse_codegen_cfs::generate_rust(&read_and_parse("std_msgs.syn"), &opts);
    assert!(out.contains("pub struct Time {"));
    assert!(out.contains("pub struct Header {"));
    assert!(out.contains("pub sec: u32,"));
    assert!(out.contains("pub seq: u32,"));
}

#[test]
fn generated_rust_geometry_msgs_compiles() {
    let opts = RustOptions::default();
    let std_msgs = synapse_codegen_cfs::generate_rust(&read_and_parse("std_msgs.syn"), &opts);
    let geometry_msgs = synapse_codegen_cfs::generate_rust(&read_and_parse("geometry_msgs.syn"), &opts);

    let src = format!(
        r#"
pub mod cfs_sys {{
    #[repr(C)]
    pub struct CFE_MSG_TelemetryHeader_t {{
        pub bytes: [u8; 16],
    }}

    #[repr(C)]
    pub struct CFE_MSG_CommandHeader_t {{
        pub bytes: [u8; 16],
    }}
}}

pub mod std_msgs {{
{std_msgs}
}}

pub mod geometry_msgs {{
    use crate::cfs_sys;
{geometry_msgs}
}}
"#
    );

    let dir = std::env::temp_dir().join(format!("synapse-rustc-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("create temp rustc dir");
    let src_path = dir.join("generated_geometry.rs");
    let lib_path = dir.join("libgenerated_geometry.rlib");
    fs::write(&src_path, src).expect("write generated Rust test source");

    let output = Command::new("rustc")
        .arg("--edition=2021")
        .arg("--crate-type=lib")
        .arg(&src_path)
        .arg("-o")
        .arg(&lib_path)
        .output()
        .expect("run rustc");

    assert!(
        output.status.success(),
        "generated Rust did not compile\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

// ── Helpers ────────────────────────────────────────────────────────────────────

fn to_screaming_snake(name: &str) -> String {
    let mut out = String::new();
    for (i, ch) in name.chars().enumerate() {
        if ch.is_uppercase() && i > 0 { out.push('_'); }
        out.push(ch.to_ascii_uppercase());
    }
    out
}

/// All 14 stamped messages with their C MID strings (U suffix).
const STAMPED_MIDS: &[(&str, &str)] = &[
    ("AccelStamped",               "0x0800U"),
    ("AccelWithCovarianceStamped", "0x0801U"),
    ("InertiaStamped",             "0x0802U"),
    ("PointStamped",               "0x0803U"),
    ("PolygonStamped",             "0x0804U"),
    ("PoseArray",                  "0x0805U"),
    ("PoseStamped",                "0x0806U"),
    ("PoseWithCovarianceStamped",  "0x0807U"),
    ("QuaternionStamped",          "0x0808U"),
    ("TransformStamped",           "0x0809U"),
    ("TwistStamped",               "0x080AU"),
    ("TwistWithCovarianceStamped", "0x080BU"),
    ("Vector3Stamped",             "0x080CU"),
    ("WrenchStamped",              "0x080DU"),
];

/// Same messages with Rust hex literals (no U suffix).
const STAMPED_HEX: &[(&str, &str)] = &[
    ("AccelStamped",               "0x0800"),
    ("AccelWithCovarianceStamped", "0x0801"),
    ("InertiaStamped",             "0x0802"),
    ("PointStamped",               "0x0803"),
    ("PolygonStamped",             "0x0804"),
    ("PoseArray",                  "0x0805"),
    ("PoseStamped",                "0x0806"),
    ("PoseWithCovarianceStamped",  "0x0807"),
    ("QuaternionStamped",          "0x0808"),
    ("TransformStamped",           "0x0809"),
    ("TwistStamped",               "0x080A"),
    ("TwistWithCovarianceStamped", "0x080B"),
    ("Vector3Stamped",             "0x080C"),
    ("WrenchStamped",              "0x080D"),
];
