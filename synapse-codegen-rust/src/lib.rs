use synapse_parser::ast::{
    ArraySuffix, BaseType, ConstDecl, EnumDef, FieldDef, Item, Literal, MessageDef, PrimitiveType,
    StructDef, SynFile, TypeExpr,
};

/// Generate Rust source code from a parsed Synapse file.
pub fn generate(file: &SynFile) -> String {
    let mut out = String::new();

    // Collect the active namespace (last namespace_decl wins, matching file convention)
    // We don't emit a `mod` block — the caller can place the output where they like.

    for item in &file.items {
        match item {
            Item::Namespace(_) | Item::Import(_) => {
                // namespace/import are semantic hints for the resolver; not emitted as code
            }
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
    let ty  = scalar_type_str(&c.ty);
    let val = literal_str(&c.value);
    out.push_str(&format!("pub const {}: {} = {};\n\n", c.name, ty, val));
}

// ── Enum ──────────────────────────────────────────────────────────────────────

fn emit_enum(out: &mut String, e: &EnumDef) {
    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n");
    out.push_str(&format!("pub enum {} {{\n", e.name));
    for v in &e.variants {
        match v.value {
            Some(n) => out.push_str(&format!("    {} = {},\n", v.name, n)),
            None    => out.push_str(&format!("    {},\n", v.name)),
        }
    }
    out.push_str("}\n\n");
}

// ── Struct ────────────────────────────────────────────────────────────────────

fn emit_struct(out: &mut String, s: &StructDef) {
    out.push_str("#[derive(Debug, Clone, PartialEq)]\n");
    out.push_str(&format!("pub struct {} {{\n", s.name));
    for f in &s.fields {
        emit_field(out, f);
    }
    out.push_str("}\n\n");
}

// ── Message ───────────────────────────────────────────────────────────────────

fn emit_message(out: &mut String, m: &MessageDef) {
    out.push_str("#[derive(Debug, Clone, PartialEq)]\n");
    out.push_str(&format!("pub struct {} {{\n", m.name));
    for f in &m.fields {
        emit_field(out, f);
    }
    out.push_str("}\n\n");
}

// ── Field ─────────────────────────────────────────────────────────────────────

fn emit_field(out: &mut String, f: &FieldDef) {
    let ty_str = field_type_str(&f.ty, f.optional);
    out.push_str(&format!("    pub {}: {},\n", f.name, ty_str));
}

// ── Type helpers ──────────────────────────────────────────────────────────────

/// Full field type string, wrapping in `Option<>` when optional.
fn field_type_str(ty: &TypeExpr, optional: bool) -> String {
    let inner = type_str(ty);
    if optional {
        format!("Option<{}>", inner)
    } else {
        inner
    }
}

/// Type string without the Option wrapper.
fn type_str(ty: &TypeExpr) -> String {
    let base = base_type_str(&ty.base);
    match &ty.array {
        None                        => base,
        Some(ArraySuffix::Dynamic)  => format!("Vec<{}>", base),
        Some(ArraySuffix::Fixed(n)) => format!("[{}; {}]", base, n),
        Some(ArraySuffix::Bounded(n)) => format!("Vec<{}>  /* max {} */", base, n),
    }
}

/// Base type with no array suffix — used for const type annotations too.
fn scalar_type_str(ty: &TypeExpr) -> String {
    base_type_str(&ty.base)
}

fn base_type_str(base: &BaseType) -> String {
    match base {
        BaseType::String           => "String".to_string(),
        BaseType::Primitive(p)     => primitive_str(*p).to_string(),
        BaseType::Ref(segments)    => segments.join("::"),
    }
}

fn primitive_str(p: PrimitiveType) -> &'static str {
    match p {
        PrimitiveType::F32   => "f32",
        PrimitiveType::F64   => "f64",
        PrimitiveType::I8    => "i8",
        PrimitiveType::I16   => "i16",
        PrimitiveType::I32   => "i32",
        PrimitiveType::I64   => "i64",
        PrimitiveType::U8    => "u8",
        PrimitiveType::U16   => "u16",
        PrimitiveType::U32   => "u32",
        PrimitiveType::U64   => "u64",
        PrimitiveType::Bool  => "bool",
        PrimitiveType::Bytes => "Vec<u8>",
    }
}

// ── Literal helpers ───────────────────────────────────────────────────────────

fn literal_str(lit: &Literal) -> String {
    match lit {
        Literal::Float(f) => {
            // Always emit with a decimal point so it's unambiguously a float literal
            let s = format!("{}", f);
            if s.contains('.') || s.contains('e') { s } else { format!("{}.0", s) }
        }
        Literal::Int(n)         => n.to_string(),
        Literal::Bool(b)        => b.to_string(),
        Literal::Str(s)         => format!("{:?}", s),   // produces Rust string literal with escapes
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
        assert_eq!(out.trim(), "pub const PI: f64 = 3.14;");
    }

    #[test]
    fn const_u32() {
        let out = codegen("const MAX: u32 = 256");
        assert_eq!(out.trim(), "pub const MAX: u32 = 256;");
    }

    #[test]
    fn const_bool() {
        let out = codegen("const FLAG: bool = true");
        assert_eq!(out.trim(), "pub const FLAG: bool = true;");
    }

    #[test]
    fn const_string() {
        let out = codegen(r#"const FRAME: string = "world""#);
        assert_eq!(out.trim(), r#"pub const FRAME: String = "world";"#);
    }

    // ── Enum ─────────────────────────────────────────────────

    #[test]
    fn enum_with_values() {
        let out = codegen("enum Status { Idle = 0  Moving = 1  Error = 2 }");
        assert!(out.contains("#[derive(Debug, Clone, Copy, PartialEq, Eq)]"));
        assert!(out.contains("pub enum Status {"));
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
        assert!(out.contains("#[derive(Debug, Clone, PartialEq)]"));
        assert!(out.contains("pub struct Point {"));
        assert!(out.contains("    pub x: f64,"));
        assert!(out.contains("    pub y: f64,"));
        assert!(out.contains("    pub z: f64,"));
    }

    #[test]
    fn struct_ref_field() {
        let out = codegen("struct Pose { position: geometry::Point  orientation: geometry::Quaternion }");
        assert!(out.contains("    pub position: geometry::Point,"));
        assert!(out.contains("    pub orientation: geometry::Quaternion,"));
    }

    // ── Message ──────────────────────────────────────────────

    #[test]
    fn message_optional_field() {
        let out = codegen("message Foo { required: i32  optional?: string }");
        assert!(out.contains("    pub required: i32,"));
        assert!(out.contains("    pub optional: Option<String>,"));
    }

    #[test]
    fn message_dynamic_array() {
        let out = codegen("message M { data: u8[] }");
        assert!(out.contains("    pub data: Vec<u8>,"));
    }

    #[test]
    fn message_fixed_array() {
        let out = codegen("message M { covariance: f64[36] }");
        assert!(out.contains("    pub covariance: [f64; 36],"));
    }

    #[test]
    fn message_bounded_array() {
        let out = codegen("message M { waypoints: Point[<=256] }");
        assert!(out.contains("    pub waypoints: Vec<Point>  /* max 256 */,"));
    }

    #[test]
    fn message_bytes_field() {
        let out = codegen("message M { payload: bytes }");
        assert!(out.contains("    pub payload: Vec<u8>,"));
    }

    #[test]
    fn message_string_bounded() {
        let out = codegen(r#"message M { label: string[<=64] }"#);
        assert!(out.contains("    pub label: Vec<String>  /* max 64 */,"));
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
        assert!(out.contains("pub enum DriveMode"));
        assert!(out.contains("pub const MAX_SPEED: f64 = 2.5;"));
        assert!(out.contains("pub struct RobotState"));
        assert!(out.contains("    pub mode: DriveMode,"));
        assert!(out.contains("    pub position: geometry::Point,"));
        assert!(out.contains("    pub battery: f32,"));
        assert!(out.contains("    pub sensor_data: Vec<u8>,"));
        assert!(out.contains("    pub error_code: Option<i32>,"));
    }

    // ── Namespace/import skipped ──────────────────────────────

    #[test]
    fn namespace_and_import_produce_no_output() {
        let out = codegen(r#"namespace foo  import "bar.syn""#);
        assert_eq!(out.trim(), "");
    }
}
