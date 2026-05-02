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

#[test]
fn cfs_patterns_parse() {
    let f = read_and_parse("cfs_patterns.syn");
    // namespace + table + command + telemetry
    assert_eq!(f.items.len(), 4);
}

#[test]
fn camera_msgs_parse() {
    let f = read_and_parse("camera_msgs.syn");
    // namespace + import + enum + 3 structs + table + 3 commands + 2 telemetry
    assert_eq!(f.items.len(), 12);
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
fn cfs_c_codegen_command_telemetry_and_table() {
    let out = synapse_codegen_cfs::generate_c(&read_and_parse("cfs_patterns.syn"));
    assert!(out.contains("#define SET_MODE_MID  0x1880U"));
    assert!(out.contains("#define NAV_STATE_MID  0x0801U"));
    assert!(out.contains("} nav_app_NavConfig_t;"));
    assert!(out.contains("} nav_app_SetMode_t;"));
    assert!(out.contains("} nav_app_NavState_t;"));
    assert!(out.contains("CFE_MSG_CommandHeader_t Header;"));
    assert!(out.contains("CFE_MSG_TelemetryHeader_t Header;"));

    let table_start = out.find("typedef struct {\n    double max_speed;").unwrap();
    let table_end = out[table_start..].find("} nav_app_NavConfig_t;").unwrap() + table_start;
    let table = &out[table_start..table_end];
    assert!(!table.contains("CFE_MSG_"));
}

#[test]
fn cfs_c_codegen_camera_msgs() {
    let out = synapse_codegen_cfs::generate_c(&read_and_parse("camera_msgs.syn"));
    assert!(out.contains("#include \"std_msgs.h\""));
    assert!(out.contains("#define SET_CAMERA_MODE_MID  0x1881U"));
    assert!(out.contains("#define SET_EXPOSURE_MID  0x1882U"));
    assert!(out.contains("#define UPDATE_INTRINSICS_MID  0x1883U"));
    assert!(out.contains("#define CAMERA_STATUS_MID  0x0881U"));
    assert!(out.contains("#define CAMERA_CALIBRATION_STATUS_MID  0x0882U"));
    assert!(out.contains("} camera_app_CameraId_t;"));
    assert!(out.contains("} camera_app_CameraCalibration_t;"));
    assert!(out.contains("    camera_app_CameraId_t camera;"));
    assert!(out.contains("    camera_app_CameraIntrinsics_t intrinsics;"));
    assert!(out.contains("    double k[9];"));
    assert!(out.contains("    double distortion[5];"));
    assert!(out.contains("CFE_MSG_CommandHeader_t Header;"));
    assert!(out.contains("CFE_MSG_TelemetryHeader_t Header;"));

    let table_start = out.find("typedef struct {\n    camera_app_CameraId_t camera;").unwrap();
    let table_end = out[table_start..].find("} camera_app_CameraCalibration_t;").unwrap() + table_start;
    let table = &out[table_start..table_end];
    assert!(!table.contains("CFE_MSG_"));
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
fn cfs_rust_codegen_command_telemetry_and_table() {
    let opts = RustOptions::default();
    let out = synapse_codegen_cfs::generate_rust(&read_and_parse("cfs_patterns.syn"), &opts);
    assert!(out.contains("pub const SET_MODE_MID: u16 = 0x1880;"));
    assert!(out.contains("pub const NAV_STATE_MID: u16 = 0x0801;"));
    assert!(out.contains("pub struct NavConfig {"));
    assert!(out.contains("pub struct SetMode {"));
    assert!(out.contains("pub struct NavState {"));
    assert!(out.contains("pub cfs_header: cfs_sys::CFE_MSG_CommandHeader_t,"));
    assert!(out.contains("pub cfs_header: cfs_sys::CFE_MSG_TelemetryHeader_t,"));

    let table_start = out.find("pub struct NavConfig {").unwrap();
    let table_end = out[table_start..].find("}\n\n").unwrap() + table_start;
    let table = &out[table_start..table_end];
    assert!(!table.contains("cfs_header"));
}

#[test]
fn cfs_rust_codegen_camera_msgs() {
    let opts = RustOptions::default();
    let out = synapse_codegen_cfs::generate_rust(&read_and_parse("camera_msgs.syn"), &opts);
    assert!(out.contains("use crate::std_msgs;"));
    assert!(out.contains("pub const SET_CAMERA_MODE_MID: u16 = 0x1881;"));
    assert!(out.contains("pub const SET_EXPOSURE_MID: u16 = 0x1882;"));
    assert!(out.contains("pub const UPDATE_INTRINSICS_MID: u16 = 0x1883;"));
    assert!(out.contains("pub const CAMERA_STATUS_MID: u16 = 0x0881;"));
    assert!(out.contains("pub const CAMERA_CALIBRATION_STATUS_MID: u16 = 0x0882;"));
    assert!(out.contains("pub struct CameraCalibration {"));
    assert!(out.contains("pub struct CameraId {"));
    assert!(out.contains("pub struct UpdateIntrinsics {"));
    assert!(out.contains("    pub camera: CameraId,"));
    assert!(out.contains("    pub k: [f64; 9],"));
    assert!(out.contains("    pub distortion: [f64; 5],"));
    assert!(out.contains("    pub cfs_header: cfs_sys::CFE_MSG_CommandHeader_t,"));
    assert!(out.contains("    pub cfs_header: cfs_sys::CFE_MSG_TelemetryHeader_t,"));

    let table_start = out.find("pub struct CameraCalibration {").unwrap();
    let table_end = out[table_start..].find("}\n\n").unwrap() + table_start;
    let table = &out[table_start..table_end];
    assert!(!table.contains("cfs_header"));
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
