use std::{fs, path::Path, process::Command};
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

// ── Rust codegen ───────────────────────────────────────────────────────────────

#[test]
fn rust_codegen_geometry_msgs() {
    let out = synapse_codegen_rust::generate(&read_and_parse("geometry_msgs.syn"));

    // Primitives
    assert!(out.contains("pub struct Vector3 {"));
    assert!(out.contains("pub struct Point {"));
    assert!(out.contains("pub struct Point32 {"));
    assert!(out.contains("pub struct Quaternion {"));

    // Covariance arrays
    assert!(out.contains("pub covariance: [f64; 36],"));

    // Dynamic arrays
    assert!(out.contains("pub points: Vec<Point32>,"));
    assert!(out.contains("pub poses: Vec<Pose>,"));

    // Cross-namespace reference
    assert!(out.contains("pub header: std_msgs::Header,"));

    // String field
    assert!(out.contains("pub child_frame_id: String,"));

    // All 29 named types present
    for name in ROS_TYPE_NAMES {
        assert!(
            out.contains(&format!("pub struct {name} {{")),
            "missing: {name}"
        );
    }
}

#[test]
fn rust_codegen_std_msgs() {
    let out = synapse_codegen_rust::generate(&read_and_parse("std_msgs.syn"));
    assert!(out.contains("pub struct Time {"));
    assert!(out.contains("pub struct Header {"));
    assert!(out.contains("pub seq: u32,"));
    assert!(out.contains("pub stamp: Time,"));
    assert!(out.contains("pub frame_id: String,"));
}

// ── C++ codegen ────────────────────────────────────────────────────────────────

#[test]
fn cpp_codegen_geometry_msgs() {
    let out = synapse_codegen_cpp::generate(&read_and_parse("geometry_msgs.syn"));

    assert!(out.contains("#pragma once"));
    assert!(out.contains("#include <stdint.h>"));

    // Primitives
    assert!(out.contains("struct Vector3 {"));
    assert!(out.contains("struct Point {"));
    assert!(out.contains("struct Quaternion {"));

    // Covariance arrays
    assert!(out.contains("    double covariance[36];"));

    // Dynamic arrays
    assert!(out.contains("    Span<Point32> points;"));
    assert!(out.contains("    Span<Pose> poses;"));

    // Cross-namespace reference
    assert!(out.contains("    std_msgs::Header header;"));

    // String field
    assert!(out.contains("    const char* child_frame_id;"));

    // All 29 named types present
    for name in ROS_TYPE_NAMES {
        assert!(out.contains(&format!("struct {name} {{")), "missing: {name}");
    }
}

#[test]
fn cpp_codegen_std_msgs() {
    let out = synapse_codegen_cpp::generate(&read_and_parse("std_msgs.syn"));
    assert!(out.contains("struct Time {"));
    assert!(out.contains("struct Header {"));
    assert!(out.contains("    uint32_t seq;"));
    assert!(out.contains("    Time stamp;"));
    assert!(out.contains("    const char* frame_id;"));
}

// ── Rust compile ───────────────────────────────────────────────────────────────

#[test]
fn rust_compiles() {
    let std_src = synapse_codegen_rust::generate(&read_and_parse("std_msgs.syn"));
    let geo_src = synapse_codegen_rust::generate(&read_and_parse("geometry_msgs.syn"));

    // Wrap in modules; geometry_msgs imports std_msgs from parent scope.
    let combined = format!(
        "#![allow(dead_code, unused_imports)]\n\
         pub mod std_msgs {{\n{std_src}\n}}\n\
         pub mod geometry_msgs {{\n    use super::std_msgs;\n{geo_src}\n}}\n"
    );

    let tmp = std::env::temp_dir().join("synapse_geo_integration.rs");
    fs::write(&tmp, &combined).unwrap();

    let status = Command::new("rustc")
        .args(["--edition", "2021", "--crate-type", "lib", "--out-dir"])
        .arg(std::env::temp_dir())
        .arg(&tmp)
        .status()
        .expect("rustc not found — is it on PATH?");

    assert!(status.success(), "Rust compilation of generated code failed");
}

// ── C++ compile ────────────────────────────────────────────────────────────────

fn find_cpp_compiler() -> Option<&'static str> {
    for cc in ["clang++", "g++"] {
        if Command::new(cc).arg("--version").output().is_ok() {
            return Some(cc);
        }
    }
    None
}

#[test]
fn cpp_compiles() {
    let Some(cc) = find_cpp_compiler() else {
        eprintln!("skipping cpp_compiles: no C++ compiler found on PATH");
        return;
    };

    let std_types = synapse_codegen_cpp::generate_types(&read_and_parse("std_msgs.syn"));
    let geo_types = synapse_codegen_cpp::generate_types(&read_and_parse("geometry_msgs.syn"));

    // One preamble, two namespace blocks. All std_msgs:: refs resolve because
    // C++ unqualified lookup for "std_msgs" walks up to the global namespace.
    let header = format!(
        "{preamble}\nnamespace std_msgs {{\n{std_types}}}\nnamespace geometry_msgs {{\n{geo_types}}}\n",
        preamble = synapse_codegen_cpp::PREAMBLE,
    );

    let tmp = std::env::temp_dir();
    let hpp = tmp.join("synapse_geo_integration.hpp");
    let cpp = tmp.join("synapse_geo_integration.cpp");

    fs::write(&hpp, &header).unwrap();
    // Use an absolute include path so the compiler doesn't need -I flags.
    fs::write(&cpp, format!("#include {:?}\n", hpp)).unwrap();

    let status = Command::new(cc)
        .args(["-std=c++11", "-fsyntax-only"])
        .arg(&cpp)
        .status()
        .expect("C++ compiler invocation failed");

    assert!(status.success(), "C++ compilation of generated code failed");
}

// ── Helpers ────────────────────────────────────────────────────────────────────

/// All 29 ROS geometry_msgs type names.
const ROS_TYPE_NAMES: &[&str] = &[
    "Accel",
    "AccelStamped",
    "AccelWithCovariance",
    "AccelWithCovarianceStamped",
    "Inertia",
    "InertiaStamped",
    "Point",
    "Point32",
    "PointStamped",
    "Polygon",
    "PolygonStamped",
    "Pose",
    "Pose2D",
    "PoseArray",
    "PoseStamped",
    "PoseWithCovariance",
    "PoseWithCovarianceStamped",
    "Quaternion",
    "QuaternionStamped",
    "Transform",
    "TransformStamped",
    "Twist",
    "TwistStamped",
    "TwistWithCovariance",
    "TwistWithCovarianceStamped",
    "Vector3",
    "Vector3Stamped",
    "Wrench",
    "WrenchStamped",
];
