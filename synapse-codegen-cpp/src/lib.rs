use synapse_parser::ast::{
    ArraySuffix, BaseType, ConstDecl, EnumDef, FieldDef, Item, Literal, MessageDef, PrimitiveType,
    StructDef, SynFile, TypeExpr,
};

pub const PREAMBLE: &str = "\
#pragma once
#include <stdint.h>
#include <stddef.h>

template<typename T>
struct Span {
    T* data;
    size_t len;
};

template<typename T>
struct Optional {
    bool has_value;
    T value;
};

";

/// Generate a standalone C++ header (preamble + type declarations).
pub fn generate(file: &SynFile) -> String {
    format!("{}{}", PREAMBLE, generate_types(file))
}

/// Generate only the type declarations — no preamble.
/// Use this when embedding generated types inside a larger file (e.g. namespace blocks).
pub fn generate_types(file: &SynFile) -> String {
    let mut out = String::new();
    for item in &file.items {
        match item {
            Item::Namespace(_) | Item::Import(_) => {}
            Item::Const(c)   => emit_const(&mut out, c),
            Item::Enum(e)    => emit_enum(&mut out, e),
            Item::Struct(s)  => emit_struct(&mut out, s),
            Item::Message(m) => emit_message(&mut out, m),
        }
    }
    out
}

// ── Const ─────────────────────────────────────────────────────────────────────

fn emit_const(out: &mut String, c: &ConstDecl) {
    let ty  = base_type_str(&c.ty.base);
    let val = literal_str(&c.value);
    out.push_str(&format!("static constexpr {} {} = {};\n\n", ty, c.name, val));
}

// ── Enum ──────────────────────────────────────────────────────────────────────

fn emit_enum(out: &mut String, e: &EnumDef) {
    out.push_str(&format!("enum class {} : int32_t {{\n", e.name));
    for v in &e.variants {
        match v.value {
            Some(n) => out.push_str(&format!("    {} = {},\n", v.name, n)),
            None    => out.push_str(&format!("    {},\n", v.name)),
        }
    }
    out.push_str("};\n\n");
}

// ── Struct ────────────────────────────────────────────────────────────────────

fn emit_struct(out: &mut String, s: &StructDef) {
    out.push_str(&format!("struct {} {{\n", s.name));
    for f in &s.fields {
        emit_field(out, f);
    }
    out.push_str("};\n\n");
}

// ── Message ───────────────────────────────────────────────────────────────────

fn emit_message(out: &mut String, m: &MessageDef) {
    out.push_str("// message\n");
    out.push_str(&format!("struct {} {{\n", m.name));
    for f in &m.fields {
        emit_field(out, f);
    }
    out.push_str("};\n\n");
}

// ── Field ─────────────────────────────────────────────────────────────────────

fn emit_field(out: &mut String, f: &FieldDef) {
    // C/C++ fixed arrays require `T name[N]` syntax; Optional<T[N]> is not valid C++
    if let Some(ArraySuffix::Fixed(n)) = &f.ty.array {
        let base = base_type_str(&f.ty.base);
        let opt  = if f.optional { "  /* optional */" } else { "" };
        out.push_str(&format!("    {} {}[{}]{};\n", base, f.name, n, opt));
        return;
    }

    let (ty, comment) = non_fixed_type_str(&f.ty);
    let suffix = if comment.is_empty() {
        String::new()
    } else {
        format!("  {}", comment)
    };

    if f.optional {
        out.push_str(&format!("    Optional<{}> {};{}\n", ty, f.name, suffix));
    } else {
        out.push_str(&format!("    {} {};{}\n", ty, f.name, suffix));
    }
}

// ── Type helpers ──────────────────────────────────────────────────────────────

/// Returns (type_string, trailing_comment) for non-fixed-array types.
/// Keeping the comment separate avoids embedding it inside template angle brackets.
fn non_fixed_type_str(ty: &TypeExpr) -> (String, String) {
    let base = base_type_str(&ty.base);
    match &ty.array {
        None                          => (base, String::new()),
        Some(ArraySuffix::Fixed(_))   => unreachable!("handled by caller"),
        Some(ArraySuffix::Dynamic)    => (format!("Span<{}>", base), String::new()),
        Some(ArraySuffix::Bounded(n)) => {
            // string[<=N] is a bounded string, not an array of strings
            if matches!(&ty.base, BaseType::String) {
                ("const char*".to_string(), format!("/* max {} */", n))
            } else {
                (format!("Span<{}>", base), format!("/* max {} */", n))
            }
        }
    }
}

fn base_type_str(base: &BaseType) -> String {
    match base {
        BaseType::String           => "const char*".to_string(),
        BaseType::Primitive(p)     => primitive_str(*p).to_string(),
        BaseType::Ref(segments)    => segments.join("::"),
    }
}

fn primitive_str(p: PrimitiveType) -> &'static str {
    match p {
        PrimitiveType::F32   => "float",
        PrimitiveType::F64   => "double",
        PrimitiveType::I8    => "int8_t",
        PrimitiveType::I16   => "int16_t",
        PrimitiveType::I32   => "int32_t",
        PrimitiveType::I64   => "int64_t",
        PrimitiveType::U8    => "uint8_t",
        PrimitiveType::U16   => "uint16_t",
        PrimitiveType::U32   => "uint32_t",
        PrimitiveType::U64   => "uint64_t",
        PrimitiveType::Bool  => "bool",
        PrimitiveType::Bytes => "Span<uint8_t>",
    }
}

// ── Literal helpers ───────────────────────────────────────────────────────────

fn literal_str(lit: &Literal) -> String {
    match lit {
        Literal::Float(f) => {
            let s = format!("{}", f);
            if s.contains('.') || s.contains('e') { s } else { format!("{}.0", s) }
        }
        Literal::Int(n)          => n.to_string(),
        Literal::Bool(b)         => b.to_string(),
        Literal::Str(s)          => format!("{:?}", s),
        Literal::Ident(segments) => segments.join("::"),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use synapse_parser::ast::parse;

    fn codegen(src: &str) -> String {
        generate(&parse(src).unwrap())
    }

    // ── Const ────────────────────────────────────────────────

    #[test]
    fn const_f64() {
        let out = codegen("const PI: f64 = 3.14");
        assert!(out.contains("static constexpr double PI = 3.14;"));
    }

    #[test]
    fn const_u32() {
        let out = codegen("const MAX: u32 = 256");
        assert!(out.contains("static constexpr uint32_t MAX = 256;"));
    }

    #[test]
    fn const_bool() {
        let out = codegen("const FLAG: bool = true");
        assert!(out.contains("static constexpr bool FLAG = true;"));
    }

    #[test]
    fn const_string() {
        let out = codegen(r#"const FRAME: string = "world""#);
        assert!(out.contains(r#"static constexpr const char* FRAME = "world";"#));
    }

    // ── Enum ─────────────────────────────────────────────────

    #[test]
    fn enum_with_values() {
        let out = codegen("enum Status { Idle = 0  Moving = 1  Error = 2 }");
        assert!(out.contains("enum class Status : int32_t {"));
        assert!(out.contains("    Idle = 0,"));
        assert!(out.contains("    Moving = 1,"));
        assert!(out.contains("    Error = 2,"));
    }

    #[test]
    fn enum_without_values() {
        let out = codegen("enum Dir { North South East West }");
        assert!(out.contains("    North,"));
        assert!(out.contains("    West,"));
        assert!(!out.contains('='));
    }

    // ── Struct ───────────────────────────────────────────────

    #[test]
    fn struct_primitive_fields() {
        let out = codegen("struct Point { x: f64  y: f64  z: f64 }");
        assert!(out.contains("struct Point {"));
        assert!(out.contains("    double x;"));
        assert!(out.contains("    double y;"));
        assert!(out.contains("    double z;"));
    }

    #[test]
    fn struct_ref_field() {
        let out = codegen("struct Pose { position: geometry::Point  orientation: geometry::Quaternion }");
        assert!(out.contains("    geometry::Point position;"));
        assert!(out.contains("    geometry::Quaternion orientation;"));
    }

    // ── Message ──────────────────────────────────────────────

    #[test]
    fn message_optional_field() {
        let out = codegen("message Foo { required: i32  optional?: string }");
        assert!(out.contains("    int32_t required;"));
        assert!(out.contains("    Optional<const char*> optional;"));
    }

    #[test]
    fn message_dynamic_array() {
        let out = codegen("message M { data: u8[] }");
        assert!(out.contains("    Span<uint8_t> data;"));
    }

    #[test]
    fn message_fixed_array() {
        let out = codegen("message M { covariance: f64[36] }");
        assert!(out.contains("    double covariance[36];"));
    }

    #[test]
    fn message_bounded_array() {
        let out = codegen("message M { waypoints: Point[<=256] }");
        assert!(out.contains("    Span<Point> waypoints;  /* max 256 */"));
    }

    #[test]
    fn message_bytes_field() {
        let out = codegen("message M { payload: bytes }");
        assert!(out.contains("    Span<uint8_t> payload;"));
    }

    #[test]
    fn message_string_bounded() {
        let out = codegen(r#"message M { label: string[<=64] }"#);
        assert!(out.contains("    const char* label;  /* max 64 */"));
    }

    #[test]
    fn message_optional_dynamic_array() {
        let out = codegen("message M { items?: u32[] }");
        assert!(out.contains("    Optional<Span<uint32_t>> items;"));
    }

    // ── Preamble ─────────────────────────────────────────────

    #[test]
    fn preamble_present() {
        let out = codegen("struct Foo { x: i32 }");
        assert!(out.contains("#pragma once"));
        assert!(out.contains("#include <stdint.h>"));
        assert!(out.contains("#include <stddef.h>"));
        assert!(out.contains("struct Span {"));
        assert!(out.contains("struct Optional {"));
    }

    // ── Full file ─────────────────────────────────────────────

    #[test]
    fn full_robot_state() {
        let src = r#"
            namespace robot
            import "geometry.syn"

            enum DriveMode { Idle = 0  Forward = 1  Error = 2 }

            const MAX_SPEED: f64 = 2.5

            message RobotState {
                mode:        DriveMode
                position:    geometry::Point
                battery:     f32
                label:       string[<=64]
                sensor_data: u8[]
                error_code?: i32
            }
        "#;

        let out = codegen(src);
        assert!(out.contains("enum class DriveMode : int32_t {"));
        assert!(out.contains("static constexpr double MAX_SPEED = 2.5;"));
        assert!(out.contains("// message"));
        assert!(out.contains("struct RobotState {"));
        assert!(out.contains("    DriveMode mode;"));
        assert!(out.contains("    geometry::Point position;"));
        assert!(out.contains("    float battery;"));
        assert!(out.contains("    const char* label;  /* max 64 */"));
        assert!(out.contains("    Span<uint8_t> sensor_data;"));
        assert!(out.contains("    Optional<int32_t> error_code;"));
    }

    #[test]
    fn namespace_and_import_produce_no_struct_output() {
        let out = codegen(r#"namespace foo  import "bar.syn""#);
        // Only the preamble should be in the output
        assert!(!out.contains("struct foo"));
        assert!(!out.contains("bar.syn"));
    }
}
