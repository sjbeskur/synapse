use std::path::Path;
use std::fs;
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
    assert!(out.contains("} Vector3_t;"));
    assert!(out.contains("} Pose_t;"));

    // Spot-check stamped message structs
    assert!(out.contains("} AccelStamped_t;"));
    assert!(out.contains("} TransformStamped_t;"));
    assert!(out.contains("} WrenchStamped_t;"));

    // Covariance fixed array
    assert!(out.contains("    double covariance[36];"));

    // Cross-namespace field (plain C, no namespace qualifier)
    assert!(out.contains("    std_msgs_Header header;"));
}

#[test]
fn cfs_c_codegen_std_msgs() {
    let out = synapse_codegen_cfs::generate_c(&read_and_parse("std_msgs.syn"));
    assert!(out.contains("} Time_t;"));
    assert!(out.contains("} Header_t;"));
    assert!(out.contains("    uint32_t sec;"));
    assert!(out.contains("    uint32_t seq;"));
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
    assert!(out.contains("pub header: cfs_sys::CFE_MSG_TelemetryHeader_t,"));
    assert!(!out.contains("pub header: cfs_sys::CFE_MSG_CommandHeader_t,"));

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
