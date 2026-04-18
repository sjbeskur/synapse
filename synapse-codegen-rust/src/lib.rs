use synapse_parser::ast::{
    ArraySuffix, BaseType, ConstDecl, EnumDef, FieldDef, Item, Literal, MessageDef, PrimitiveType,
    StructDef, SynFile, TypeExpr,
};

// ── Public API ────────────────────────────────────────────────────────────────

/// Generate `std`-based Rust source from a parsed Synapse file.
/// Uses `String`, `Vec<T>`, and `#[derive(Debug, Clone, PartialEq)]`.
pub fn generate(file: &SynFile) -> String {
    emit_items(file, Mode::Std)
}

/// Generate `no_std`, no-alloc Rust source from a parsed Synapse file.
///
/// Replaces all heap types with a `Slice<T>` (ptr + len) that mirrors
/// `Span<T>` in the generated C++ headers.  Emits `#![no_std]` and a
/// `Slice<T>` definition in the preamble.
pub fn generate_nostd(file: &SynFile) -> String {
    let mut out = String::from(NOSTD_PREAMBLE);
    out.push_str(&emit_items(file, Mode::NoStd));
    out
}

/// Preamble emitted at the top of every `generate_nostd` output.
/// Exposed so callers that combine multiple generated files can include it once.
pub const NOSTD_PREAMBLE: &str = "\
#![no_std]

/// Raw slice — pointer and length, no heap allocation.
/// Mirrors `Span<T>` in the generated C++ headers.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct Slice<T> {
    pub ptr: *const T,
    pub len: usize,
}

";

// ── Mode ──────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode { Std, NoStd }

// ── Item emission ─────────────────────────────────────────────────────────────

fn emit_items(file: &SynFile, mode: Mode) -> String {
    let mut out = String::new();
    for item in &file.items {
        match item {
            Item::Namespace(_) | Item::Import(_) => {}
            Item::Const(c)   => emit_const(&mut out, c, mode),
            Item::Enum(e)    => emit_enum(&mut out, e),
            Item::Struct(s)  => emit_struct(&mut out, s, mode),
            Item::Message(m) => emit_message(&mut out, m, mode),
        }
    }
    out
}

// ── Const ─────────────────────────────────────────────────────────────────────

fn emit_const(out: &mut String, c: &ConstDecl, mode: Mode) {
    emit_doc(out, &c.doc, "");
    let ty  = const_type_str(&c.ty, mode);
    let val = literal_str(&c.value);
    out.push_str(&format!("pub const {}: {} = {};\n\n", c.name, ty, val));
}

// ── Enum ──────────────────────────────────────────────────────────────────────

fn emit_enum(out: &mut String, e: &EnumDef) {
    emit_doc(out, &e.doc, "");
    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n");
    out.push_str(&format!("pub enum {} {{\n", e.name));
    for v in &e.variants {
        emit_doc(out, &v.doc, "    ");
        match v.value {
            Some(n) => out.push_str(&format!("    {} = {},\n", v.name, n)),
            None    => out.push_str(&format!("    {},\n", v.name)),
        }
    }
    out.push_str("}\n\n");
}

// ── Struct ────────────────────────────────────────────────────────────────────

fn emit_struct(out: &mut String, s: &StructDef, mode: Mode) {
    emit_doc(out, &s.doc, "");
    out.push_str(struct_derive(mode));
    out.push_str(&format!("pub struct {} {{\n", s.name));
    for f in &s.fields {
        emit_field(out, f, mode);
    }
    out.push_str("}\n\n");
}

// ── Message ───────────────────────────────────────────────────────────────────

fn emit_message(out: &mut String, m: &MessageDef, mode: Mode) {
    emit_doc(out, &m.doc, "");
    out.push_str(struct_derive(mode));
    out.push_str(&format!("pub struct {} {{\n", m.name));
    for f in &m.fields {
        emit_field(out, f, mode);
    }
    out.push_str("}\n\n");
}

fn struct_derive(mode: Mode) -> &'static str {
    match mode {
        // no_std: Slice<T> has no Debug/PartialEq; Copy works since ptr+len are Copy.
        Mode::NoStd => "#[derive(Clone, Copy)]\n",
        Mode::Std   => "#[derive(Debug, Clone, PartialEq)]\n",
    }
}

// ── Field ─────────────────────────────────────────────────────────────────────

fn emit_field(out: &mut String, f: &FieldDef, mode: Mode) {
    emit_doc(out, &f.doc, "    ");
    let ty_str = field_type_str(&f.ty, f.optional, mode);
    out.push_str(&format!("    pub {}: {},\n", f.name, ty_str));
}

// ── Doc helpers ───────────────────────────────────────────────────────────────

fn emit_doc(out: &mut String, doc: &[String], indent: &str) {
    for line in doc {
        if line.is_empty() {
            out.push_str(&format!("{indent}///\n"));
        } else {
            out.push_str(&format!("{indent}/// {line}\n"));
        }
    }
}

// ── Type helpers ──────────────────────────────────────────────────────────────

fn field_type_str(ty: &TypeExpr, optional: bool, mode: Mode) -> String {
    let inner = type_str(ty, mode);
    if optional { format!("Option<{}>", inner) } else { inner }
}

fn type_str(ty: &TypeExpr, mode: Mode) -> String {
    let base = base_type_str(&ty.base, mode);
    match &ty.array {
        None => base,
        Some(ArraySuffix::Fixed(n))   => format!("[{}; {}]", base, n),
        Some(ArraySuffix::Dynamic)    => wrap_dynamic(base, mode),
        Some(ArraySuffix::Bounded(n)) => {
            // string[<=N] is a bounded string, not an array of strings
            if matches!(&ty.base, BaseType::String) {
                match mode {
                    Mode::Std   => format!("Vec<String>  /* max {} */", n),
                    Mode::NoStd => format!("Slice<u8>  /* max {} */", n),
                }
            } else {
                format!("{}  /* max {} */", wrap_dynamic(base, mode), n)
            }
        }
    }
}

fn wrap_dynamic(base: String, mode: Mode) -> String {
    match mode {
        Mode::Std   => format!("Vec<{}>", base),
        Mode::NoStd => format!("Slice<{}>", base),
    }
}

/// Type string for const declarations — scalar only, no array suffix.
fn const_type_str(ty: &TypeExpr, mode: Mode) -> String {
    match &ty.base {
        // Consts are always static; &'static str works in no_std
        BaseType::String => match mode {
            Mode::Std   => "String".to_string(),
            Mode::NoStd => "&'static str".to_string(),
        },
        other => base_type_str(other, mode),
    }
}

fn base_type_str(base: &BaseType, mode: Mode) -> String {
    match base {
        BaseType::String        => match mode {
            Mode::Std   => "String".to_string(),
            Mode::NoStd => "Slice<u8>".to_string(),
        },
        BaseType::Primitive(p)  => primitive_str(*p, mode).to_string(),
        BaseType::Ref(segments) => segments.join("::"),
    }
}

fn primitive_str(p: PrimitiveType, mode: Mode) -> &'static str {
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
        // bytes is always a raw byte buffer; in no_std use Slice<u8>
        PrimitiveType::Bytes => match mode {
            Mode::Std   => "Vec<u8>",
            Mode::NoStd => "Slice<u8>",
        },
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
        Literal::Hex(n)          => format!("0x{:X}", n),
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

    fn codegen(src: &str) -> String { generate(&parse(src).unwrap()) }
    fn codegen_ns(src: &str) -> String { generate_nostd(&parse(src).unwrap()) }

    // ── Const (std) ──────────────────────────────────────────

    #[test]
    fn const_f64() {
        assert_eq!(codegen("const PI: f64 = 3.14").trim(), "pub const PI: f64 = 3.14;");
    }

    #[test]
    fn const_u32() {
        assert_eq!(codegen("const MAX: u32 = 256").trim(), "pub const MAX: u32 = 256;");
    }

    #[test]
    fn const_bool() {
        assert_eq!(codegen("const FLAG: bool = true").trim(), "pub const FLAG: bool = true;");
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

    // ── Struct (std) ─────────────────────────────────────────

    #[test]
    fn struct_primitive_fields() {
        let out = codegen("struct Point { x: f64  y: f64  z: f64 }");
        assert!(out.contains("#[derive(Debug, Clone, PartialEq)]"));
        assert!(out.contains("pub struct Point {"));
        assert!(out.contains("    pub x: f64,"));
    }

    #[test]
    fn struct_ref_field() {
        let out = codegen("struct Pose { position: geometry::Point  orientation: geometry::Quaternion }");
        assert!(out.contains("    pub position: geometry::Point,"));
        assert!(out.contains("    pub orientation: geometry::Quaternion,"));
    }

    // ── Message (std) ────────────────────────────────────────

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

    // ── no_std ───────────────────────────────────────────────

    #[test]
    fn nostd_preamble() {
        let out = codegen_ns("struct Foo { x: i32 }");
        assert!(out.contains("#![no_std]"));
        assert!(out.contains("pub struct Slice<T>"));
        assert!(out.contains("pub ptr: *const T,"));
        assert!(out.contains("pub len: usize,"));
    }

    #[test]
    fn nostd_derives() {
        let out = codegen_ns("struct Point { x: f64 }");
        assert!(out.contains("#[derive(Clone, Copy)]"));
        assert!(!out.contains("Debug"));
        assert!(!out.contains("PartialEq"));
        // enums still get full derives
        let out2 = codegen_ns("enum E { A B }");
        assert!(out2.contains("#[derive(Debug, Clone, Copy, PartialEq, Eq)]"));
    }

    #[test]
    fn nostd_vec_replaced() {
        let out = codegen_ns("message M { data: u8[]  payload: bytes  label: string }");
        assert!(out.contains("    pub data: Slice<u8>,"));
        assert!(out.contains("    pub payload: Slice<u8>,"));
        assert!(out.contains("    pub label: Slice<u8>,"));
        assert!(!out.contains("Vec"));
        assert!(!out.contains("String"));
    }

    #[test]
    fn nostd_fixed_array_unchanged() {
        let out = codegen_ns("message M { covariance: f64[36] }");
        assert!(out.contains("    pub covariance: [f64; 36],"));
    }

    #[test]
    fn nostd_bounded_array() {
        let out = codegen_ns("message M { waypoints: Point[<=256]  label: string[<=64] }");
        assert!(out.contains("    pub waypoints: Slice<Point>  /* max 256 */,"));
        assert!(out.contains("    pub label: Slice<u8>  /* max 64 */,"));
    }

    #[test]
    fn nostd_const_string() {
        let out = codegen_ns(r#"const FRAME: string = "world""#);
        assert!(out.contains(r#"pub const FRAME: &'static str = "world";"#));
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

    #[test]
    fn namespace_and_import_produce_no_output() {
        let out = codegen(r#"namespace foo  import "bar.syn""#);
        assert_eq!(out.trim(), "");
    }
}
