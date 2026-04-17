pub mod ast;

pub mod synapse {
    use pest_derive::Parser;

    #[derive(Parser)]
    #[grammar = "synapse.pest"]
    pub struct SynapseParser;
}

pub use synapse::SynapseParser;

#[cfg(test)]
mod synapse_tests {
    use super::synapse::{Rule, SynapseParser};
    use pest::Parser;

    fn parses(rule: Rule, input: &str) -> bool {
        SynapseParser::parse(rule, input)
            .map(|mut p| p.next().map_or(false, |pair| pair.as_span().end() == input.len()))
            .unwrap_or(false)
    }

    fn parses_file(input: &str) -> bool {
        parses(Rule::file, input)
    }

    // =========================================================
    // Identifiers and scoped names
    // =========================================================

    #[test]
    fn ident_basic() {
        assert!(parses(Rule::ident, "foo"));
        assert!(parses(Rule::ident, "MyType"));
        assert!(parses(Rule::ident, "snake_case"));
        assert!(parses(Rule::ident, "CamelCase123"));
    }

    #[test]
    fn ident_rejects_leading_digit() {
        assert!(!parses(Rule::ident, "1bad"));
    }

    #[test]
    fn scoped_ident_bare() {
        assert!(parses(Rule::scoped_ident, "Point"));
    }

    #[test]
    fn scoped_ident_qualified() {
        assert!(parses(Rule::scoped_ident, "geometry::Point"));
        assert!(parses(Rule::scoped_ident, "nav::msgs::Odometry"));
    }

    // =========================================================
    // Literals
    // =========================================================

    #[test]
    fn int_literals() {
        assert!(parses(Rule::int_lit, "0"));
        assert!(parses(Rule::int_lit, "42"));
        assert!(parses(Rule::int_lit, "-7"));
    }

    #[test]
    fn float_literals() {
        assert!(parses(Rule::float_lit, "3.14"));
        assert!(parses(Rule::float_lit, "-2.5"));
        assert!(parses(Rule::float_lit, "1.5e-3"));
        assert!(parses(Rule::float_lit, "1e10"));
        assert!(!parses(Rule::float_lit, "42"));   // bare int is not a float
    }

    #[test]
    fn bool_literals() {
        assert!(parses(Rule::bool_lit, "true"));
        assert!(parses(Rule::bool_lit, "false"));
        assert!(!parses(Rule::bool_lit, "True"));
        assert!(!parses(Rule::bool_lit, "TRUE"));
    }

    #[test]
    fn string_literals() {
        assert!(parses(Rule::string_lit, r#""hello""#));
        assert!(parses(Rule::string_lit, r#""""#));
        assert!(parses(Rule::string_lit, r#""escaped\"quote""#));
    }

    #[test]
    fn ident_literal_for_enum_refs() {
        assert!(parses(Rule::ident_lit, "Idle"));
        assert!(parses(Rule::ident_lit, "DriveMode::Idle"));
        assert!(parses(Rule::ident_lit, "pkg::Status::Active"));
    }

    // =========================================================
    // Types
    // =========================================================

    #[test]
    fn primitive_types() {
        for t in &["f32", "f64", "i8", "i16", "i32", "i64",
                   "u8", "u16", "u32", "u64", "bool", "bytes"] {
            assert!(parses(Rule::primitive_type, t), "failed: {t}");
        }
    }

    #[test]
    fn string_type() {
        assert!(parses(Rule::string_type, "string"));
    }

    #[test]
    fn type_ref_bare_and_qualified() {
        assert!(parses(Rule::type_ref, "Point"));
        assert!(parses(Rule::type_ref, "geometry::Point"));
    }

    #[test]
    fn type_expr_scalar() {
        assert!(parses(Rule::type_expr, "f64"));
        assert!(parses(Rule::type_expr, "bool"));
        assert!(parses(Rule::type_expr, "string"));
        assert!(parses(Rule::type_expr, "Point"));
        assert!(parses(Rule::type_expr, "geometry::Point"));
    }

    #[test]
    fn type_expr_dynamic_array() {
        assert!(parses(Rule::type_expr, "f64[]"));
        assert!(parses(Rule::type_expr, "u8[]"));
        assert!(parses(Rule::type_expr, "string[]"));
        assert!(parses(Rule::type_expr, "geometry::Point[]"));
    }

    #[test]
    fn type_expr_fixed_array() {
        assert!(parses(Rule::type_expr, "f64[3]"));
        assert!(parses(Rule::type_expr, "u8[256]"));
        assert!(parses(Rule::type_expr, "f64[36]"));
    }

    #[test]
    fn type_expr_bounded_array() {
        assert!(parses(Rule::type_expr, "u8[<=256]"));
        assert!(parses(Rule::type_expr, "string[<=64]"));    // bounded string
        assert!(parses(Rule::type_expr, "geometry::Point[<=100]"));
    }

    // =========================================================
    // Enum
    // =========================================================

    #[test]
    fn enum_with_explicit_values() {
        assert!(parses_file(
            "enum Status { Idle = 0  Moving = 1  Error = 2 }"
        ));
    }

    #[test]
    fn enum_without_values() {
        assert!(parses_file(
            "enum Direction { North South East West }"
        ));
    }

    #[test]
    fn enum_multiline() {
        assert!(parses_file(
            "enum DriveMode {\n    Idle    = 0\n    Forward = 1\n    Error   = 2\n}"
        ));
    }

    #[test]
    fn enum_mixed_values() {
        assert!(parses_file(
            "enum Mixed { A  B = 5  C  D = 10 }"
        ));
    }

    // =========================================================
    // Struct
    // =========================================================

    #[test]
    fn struct_basic() {
        assert!(parses_file(
            "struct Point { x: f64  y: f64  z: f64 }"
        ));
    }

    #[test]
    fn struct_with_defaults() {
        assert!(parses_file(
            "struct Point { x: f64 = 0.0  y: f64 = 0.0  z: f64 = 0.0 }"
        ));
    }

    #[test]
    fn struct_empty() {
        assert!(parses_file("struct Empty {}"));
    }

    #[test]
    fn struct_multiline() {
        assert!(parses_file(
            "struct Pose {\n    position: Point\n    orientation: Quaternion\n}"
        ));
    }

    // =========================================================
    // Message
    // =========================================================

    #[test]
    fn message_basic() {
        assert!(parses_file(
            "message Ping { seq: u32  stamp: u64 }"
        ));
    }

    #[test]
    fn message_optional_field() {
        assert!(parses_file(
            "message Foo { required: i32  optional?: string }"
        ));
    }

    #[test]
    fn message_with_defaults() {
        assert!(parses_file(
            r#"message Config { label: string[<=64] = "default"  retries: u8 = 3  verbose: bool = false }"#
        ));
    }

    #[test]
    fn message_array_fields() {
        assert!(parses_file(
            "message Data { raw: u8[]  fixed: f64[3]  bounded: u8[<=256] }"
        ));
    }

    #[test]
    fn message_nested_types() {
        assert!(parses_file(
            "message Odom { position: Point  velocity: Twist  path: Point[] }"
        ));
    }

    #[test]
    fn message_qualified_types() {
        assert!(parses_file(
            "message Pose { position: geometry::Point  orientation: geometry::Quaternion }"
        ));
    }

    #[test]
    fn message_enum_default() {
        assert!(parses_file(
            "message State { mode: DriveMode = DriveMode::Idle  speed: f64 = 0.0 }"
        ));
    }

    // =========================================================
    // Const
    // =========================================================

    #[test]
    fn const_float() {
        assert!(parses_file("const PI: f64 = 3.14159265358979"));
        assert!(parses_file("const G: f32 = 9.81"));
    }

    #[test]
    fn const_int() {
        assert!(parses_file("const MAX_SIZE: u32 = 100"));
        assert!(parses_file("const MIN: i32 = -32768"));
    }

    #[test]
    fn const_bool() {
        assert!(parses_file("const DEBUG: bool = true"));
    }

    #[test]
    fn const_string() {
        assert!(parses_file(r#"const FRAME: string = "world""#));
    }

    // =========================================================
    // Namespace and import
    // =========================================================

    #[test]
    fn namespace_bare() {
        assert!(parses_file("namespace geometry\nstruct Point { x: f64  y: f64 }"));
    }

    #[test]
    fn namespace_qualified() {
        assert!(parses_file("namespace nav::msgs\nmessage Odom { x: f64 }"));
    }

    #[test]
    fn import_decl() {
        assert!(parses_file(r#"import "geometry.syn""#));
        assert!(parses_file(r#"import "common/types.syn""#));
    }

    // =========================================================
    // Comments
    // =========================================================

    #[test]
    fn line_comments() {
        assert!(parses_file(
            "// top comment\nstruct S { x: f64 // x coord\n}"
        ));
    }

    #[test]
    fn block_comments() {
        assert!(parses_file(
            "/* header */ struct S { /* inline */ x: f64 }"
        ));
    }

    // =========================================================
    // Full realistic files
    // =========================================================

    #[test]
    fn geometry_file() {
        assert!(parses_file(
            "namespace geometry

            struct Point {
                x: f64 = 0.0
                y: f64 = 0.0
                z: f64 = 0.0
            }

            struct Quaternion {
                x: f64 = 0.0
                y: f64 = 0.0
                z: f64 = 0.0
                w: f64 = 1.0
            }

            struct Pose {
                position:    Point
                orientation: Quaternion
            }"
        ));
    }

    #[test]
    fn robot_state_file() {
        assert!(parses_file(
            r#"import "geometry.syn"

            enum DriveMode {
                Idle    = 0
                Forward = 1
                Turning = 2
                Error   = 3
            }

            const MAX_SPEED: f64 = 2.5

            message RobotState {
                mode:         DriveMode         = DriveMode::Idle
                position:     geometry::Point
                velocity:     geometry::Point
                battery:      f32               = 100.0
                label:        string[<=64]       = "robot"
                sensor_data:  u8[]
                waypoints:    geometry::Point[]
                error_code?:  i32
            }"#
        ));
    }

    // =========================================================
    // Rejection tests
    // =========================================================

    #[test]
    fn rejects_field_without_type() {
        assert!(!parses_file("struct S { x }"));
    }

    #[test]
    fn rejects_missing_closing_brace() {
        assert!(!parses_file("struct S { x: f64"));
    }

    #[test]
    fn rejects_unknown_top_level() {
        assert!(!parses_file("typedef i32 MyInt"));
    }
}
