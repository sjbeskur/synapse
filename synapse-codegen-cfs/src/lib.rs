use synapse_parser::ast::{
    ArraySuffix, Attribute, BaseType, ConstDecl, Item, Literal, MessageDef, PrimitiveType,
    SynFile, TypeExpr,
};

// ── Public API ────────────────────────────────────────────────────────────────

/// Preamble included at the top of generated cFS C headers.
pub const PREAMBLE: &str = "\
#pragma once
#include \"cfe.h\"

";

/// Options for Rust cFS binding generation.
pub struct RustOptions<'a> {
    /// Module path prefix for the cFS header types.
    /// e.g. `"cfs"` → `cfs::TelemetryHeader`, `"cfe_sys"` → `cfe_sys::TelemetryHeader`.
    /// Set to `""` to use bare type names.
    pub cfs_module: &'a str,
    /// Rust type name for telemetry message headers. Default: `"TelemetryHeader"`.
    pub tlm_header: &'a str,
    /// Rust type name for command message headers. Default: `"CommandHeader"`.
    pub cmd_header: &'a str,
}

impl Default for RustOptions<'_> {
    fn default() -> Self {
        RustOptions {
            cfs_module: "cfs",
            tlm_header: "TelemetryHeader",
            cmd_header: "CommandHeader",
        }
    }
}

/// Generate a complete NASA cFS C header from a parsed Synapse file.
pub fn generate(file: &SynFile) -> String {
    let mut out = String::from(PREAMBLE);
    emit_items(file, &mut out);
    out
}

/// Generate `#[repr(C)]` Rust structs compatible with NASA cFS message bindings.
///
/// Each `message` becomes a struct with the cFS header as the first field,
/// matching the C ABI layout. MID constants are emitted as `pub const`.
pub fn generate_rust(file: &SynFile, opts: &RustOptions) -> String {
    let mut out = String::new();
    emit_rust_items(file, opts, &mut out);
    out
}

// ── Item emission ─────────────────────────────────────────────────────────────

fn emit_items(file: &SynFile, out: &mut String) {
    // First pass: emit #define MID lines for messages with @mid
    let mut has_mids = false;
    for item in &file.items {
        if let Item::Message(m) = item {
            if let Some(mid) = find_mid_attr(&m.attrs) {
                if !has_mids {
                    out.push_str("/* Message IDs */\n");
                    has_mids = true;
                }
                let define_name = to_screaming_snake(&m.name);
                let mid_str = literal_mid_str(mid);
                out.push_str(&format!("#define {}_MID  {}\n", define_name, mid_str));
            }
        }
    }
    if has_mids { out.push('\n'); }

    // Second pass: emit const, enum, struct, message types
    for item in &file.items {
        match item {
            Item::Namespace(_) | Item::Import(_) => {}
            Item::Const(c)   => emit_const(out, c),
            Item::Enum(_)    => {} // enums not needed in cFS message headers
            Item::Struct(_)  => {} // plain structs not emitted; only messages get cFS wrappers
            Item::Message(m) => emit_message(out, m),
        }
    }
}

// ── Const ─────────────────────────────────────────────────────────────────────

fn emit_const(out: &mut String, c: &ConstDecl) {
    let val = literal_str(&c.value);
    out.push_str(&format!("#define {}  {}\n\n", c.name, val));
}

// ── Message ───────────────────────────────────────────────────────────────────

fn emit_message(out: &mut String, m: &MessageDef) {
    let header_type = if is_command(m) {
        "CFE_MSG_CommandHeader_t"
    } else {
        "CFE_MSG_TelemetryHeader_t"
    };

    for line in &m.doc {
        if line.is_empty() {
            out.push_str("///\n");
        } else {
            out.push_str(&format!("/// {line}\n"));
        }
    }

    out.push_str(&format!("typedef struct {{\n"));
    out.push_str(&format!("    {} Header;\n", header_type));
    for f in &m.fields {
        if let Some(ArraySuffix::Fixed(n)) = &f.ty.array {
            let base = base_type_str(&f.ty.base);
            out.push_str(&format!("    {} {}[{}];\n", base, f.name, n));
        } else {
            let ty = non_fixed_type_str(&f.ty);
            out.push_str(&format!("    {} {};\n", ty, f.name));
        }
    }
    out.push_str(&format!("}} {}_t;\n\n", m.name));
}

// ── Rust emission ─────────────────────────────────────────────────────────────

fn emit_rust_items(file: &SynFile, opts: &RustOptions, out: &mut String) {
    // First pass: MID consts for messages with @mid
    let mut has_mids = false;
    for item in &file.items {
        if let Item::Message(m) = item {
            if let Some(mid) = find_mid_attr(&m.attrs) {
                if !has_mids {
                    out.push_str("// Message IDs\n");
                    has_mids = true;
                }
                let const_name = format!("{}_MID", to_screaming_snake(&m.name));
                let val = rust_mid_str(mid);
                out.push_str(&format!("pub const {}: u16 = {};\n", const_name, val));
            }
        }
    }
    if has_mids { out.push('\n'); }

    // Second pass: types
    for item in &file.items {
        match item {
            Item::Namespace(_) | Item::Import(_) => {}
            Item::Const(c)   => emit_rust_const(out, c),
            Item::Enum(_)    => {}
            Item::Struct(_)  => {}
            Item::Message(m) => emit_rust_message(out, m, opts),
        }
    }
}

fn emit_rust_const(out: &mut String, c: &ConstDecl) {
    let val = rust_literal_str(&c.value);
    out.push_str(&format!("pub const {}: u16 = {};\n\n", c.name, val));
}

fn emit_rust_message(out: &mut String, m: &MessageDef, opts: &RustOptions) {
    let header_type = if is_command(m) { opts.cmd_header } else { opts.tlm_header };
    let qualified = if opts.cfs_module.is_empty() {
        header_type.to_string()
    } else {
        format!("{}::{}", opts.cfs_module, header_type)
    };

    for line in &m.doc {
        if line.is_empty() {
            out.push_str("///\n");
        } else {
            out.push_str(&format!("/// {line}\n"));
        }
    }

    out.push_str("#[repr(C)]\n");
    out.push_str(&format!("pub struct {} {{\n", m.name));
    out.push_str(&format!("    pub header: {},\n", qualified));
    for f in &m.fields {
        let ty = rust_field_type_str(&f.ty);
        out.push_str(&format!("    pub {}: {},\n", f.name, ty));
    }
    out.push_str("}\n\n");
}

fn rust_field_type_str(ty: &TypeExpr) -> String {
    let base = rust_base_type_str(&ty.base);
    match &ty.array {
        None                        => base,
        Some(ArraySuffix::Fixed(n)) => format!("[{}; {}]", base, n),
        // Dynamic/bounded: use a raw slice pointer — no alloc in cFS context
        Some(ArraySuffix::Dynamic)    => format!("*const {}", base),
        Some(ArraySuffix::Bounded(n)) => format!("*const {}  /* max {} */", base, n),
    }
}

fn rust_base_type_str(base: &BaseType) -> String {
    match base {
        BaseType::String        => "*const u8".to_string(),
        BaseType::Primitive(p)  => rust_primitive_str(*p).to_string(),
        BaseType::Ref(segments) => segments.join("::"),
    }
}

fn rust_primitive_str(p: PrimitiveType) -> &'static str {
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
        PrimitiveType::Bytes => "*const u8",
    }
}

fn rust_mid_str(lit: &Literal) -> String {
    match lit {
        Literal::Hex(n) => format!("0x{:04X}", n),
        Literal::Int(n) => n.to_string(),
        Literal::Ident(segs) => segs.join("::"),
        other => rust_literal_str(other),
    }
}

fn rust_literal_str(lit: &Literal) -> String {
    match lit {
        Literal::Hex(n)          => format!("0x{:X}", n),
        Literal::Int(n)          => n.to_string(),
        Literal::Bool(b)         => b.to_string(),
        Literal::Float(f)        => {
            let s = format!("{}", f);
            if s.contains('.') || s.contains('e') { s } else { format!("{}.0", s) }
        }
        Literal::Str(s)          => format!("{:?}", s),
        Literal::Ident(segments) => segments.join("::"),
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Returns the `@mid` attribute value, if present.
fn find_mid_attr(attrs: &[Attribute]) -> Option<&Literal> {
    attrs.iter().find(|a| a.name == "mid").map(|a| &a.value)
}

/// A message is a command if its MID has bit 12 (0x1000) set, or if it has `@cmd`.
fn is_command(m: &MessageDef) -> bool {
    if m.attrs.iter().any(|a| a.name == "cmd") {
        return true;
    }
    if let Some(mid) = find_mid_attr(&m.attrs) {
        if let Some(n) = literal_to_u64(mid) {
            return (n & 0x1000) != 0;
        }
    }
    false
}

fn literal_to_u64(lit: &Literal) -> Option<u64> {
    match lit {
        Literal::Hex(n) => Some(*n),
        Literal::Int(n) if *n >= 0 => Some(*n as u64),
        _ => None,
    }
}

/// Format a MID literal for a `#define` line.
fn literal_mid_str(lit: &Literal) -> String {
    match lit {
        Literal::Hex(n) => format!("0x{:04X}U", n),
        Literal::Int(n) => format!("{}U", n),
        Literal::Ident(segs) => segs.join("::"),
        other => literal_str(other),
    }
}

fn literal_str(lit: &Literal) -> String {
    match lit {
        Literal::Float(f) => {
            let s = format!("{}", f);
            if s.contains('.') || s.contains('e') { s } else { format!("{}.0", s) }
        }
        Literal::Int(n)          => n.to_string(),
        Literal::Hex(n)          => format!("0x{:X}U", n),
        Literal::Bool(b)         => if *b { "1".to_string() } else { "0".to_string() },
        Literal::Str(s)          => format!("{:?}", s),
        Literal::Ident(segments) => segments.join("::"),
    }
}

fn non_fixed_type_str(ty: &TypeExpr) -> String {
    let base = base_type_str(&ty.base);
    match &ty.array {
        None                          => base,
        Some(ArraySuffix::Fixed(_))   => unreachable!("handled by caller"),
        Some(ArraySuffix::Dynamic)    => format!("CFE_Span_t /* {} */", base),
        Some(ArraySuffix::Bounded(n)) => format!("CFE_Span_t /* {} max {} */", base, n),
    }
}

fn base_type_str(base: &BaseType) -> String {
    match base {
        BaseType::String        => "const char*".to_string(),
        BaseType::Primitive(p)  => primitive_str(*p).to_string(),
        BaseType::Ref(segments) => segments.join("_"),
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
        PrimitiveType::Bytes => "uint8_t*",
    }
}

/// Convert `PascalCase` → `PASCAL_CASE` (screaming snake case).
fn to_screaming_snake(name: &str) -> String {
    let mut out = String::new();
    for (i, ch) in name.chars().enumerate() {
        if ch.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(ch.to_ascii_uppercase());
    }
    out
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use synapse_parser::ast::parse;

    fn codegen(src: &str) -> String { generate(&parse(src).unwrap()) }

    #[test]
    fn tlm_message_with_hex_mid() {
        let out = codegen("@mid(0x0801)\nmessage NavTlm { x: f64  y: f64 }");
        assert!(out.contains("#define NAV_TLM_MID  0x0801U"));
        assert!(out.contains("CFE_MSG_TelemetryHeader_t Header;"));
        assert!(out.contains("typedef struct {"));
        assert!(out.contains("} NavTlm_t;"));
        assert!(out.contains("    double x;"));
        assert!(out.contains("    double y;"));
    }

    #[test]
    fn cmd_message_detected_by_mid_bit12() {
        let out = codegen("@mid(0x1880)\nmessage NavCmd { seq: u16 }");
        assert!(out.contains("#define NAV_CMD_MID  0x1880U"));
        assert!(out.contains("CFE_MSG_CommandHeader_t Header;"));
        assert!(out.contains("} NavCmd_t;"));
    }

    #[test]
    fn message_without_mid_no_define() {
        let out = codegen("message Bare { x: f32 }");
        assert!(!out.contains("#define"));
        assert!(out.contains("typedef struct {"));
        assert!(out.contains("CFE_MSG_TelemetryHeader_t Header;"));
    }

    #[test]
    fn const_emits_define() {
        let out = codegen("const NAV_TLM_MID: u16 = 0x0801");
        assert!(out.contains("#define NAV_TLM_MID  0x801U"));
    }

    #[test]
    fn fixed_array_field() {
        let out = codegen("@mid(0x0802)\nmessage Imu { covariance: f64[9] }");
        assert!(out.contains("    double covariance[9];"));
    }

    // ── Rust codegen ─────────────────────────────────────────

    fn rust_codegen(src: &str) -> String {
        generate_rust(&parse(src).unwrap(), &RustOptions::default())
    }

    #[test]
    fn rust_tlm_struct() {
        let out = rust_codegen("@mid(0x0801)\nmessage NavTlm { x: f64  y: f64 }");
        assert!(out.contains("pub const NAV_TLM_MID: u16 = 0x0801;"));
        assert!(out.contains("#[repr(C)]"));
        assert!(out.contains("pub struct NavTlm {"));
        assert!(out.contains("    pub header: cfs::TelemetryHeader,"));
        assert!(out.contains("    pub x: f64,"));
        assert!(out.contains("    pub y: f64,"));
    }

    #[test]
    fn rust_cmd_struct() {
        let out = rust_codegen("@mid(0x1880)\nmessage NavCmd { seq: u16 }");
        assert!(out.contains("pub const NAV_CMD_MID: u16 = 0x1880;"));
        assert!(out.contains("    pub header: cfs::CommandHeader,"));
    }

    #[test]
    fn rust_fixed_array() {
        let out = rust_codegen("@mid(0x0802)\nmessage Imu { covariance: f64[9] }");
        assert!(out.contains("    pub covariance: [f64; 9],"));
    }

    #[test]
    fn rust_custom_module() {
        let opts = RustOptions { cfs_module: "cfe_sys", ..Default::default() };
        let out = generate_rust(&parse("@mid(0x0801)\nmessage T { x: f32 }").unwrap(), &opts);
        assert!(out.contains("cfe_sys::TelemetryHeader"));
    }

    #[test]
    fn rust_bare_module() {
        let opts = RustOptions { cfs_module: "", ..Default::default() };
        let out = generate_rust(&parse("@mid(0x0801)\nmessage T { x: f32 }").unwrap(), &opts);
        assert!(out.contains("    pub header: TelemetryHeader,"));
        assert!(!out.contains("::TelemetryHeader"));
    }

    #[test]
    fn screaming_snake_conversion() {
        assert_eq!(to_screaming_snake("NavTelemetry"), "NAV_TELEMETRY");
        assert_eq!(to_screaming_snake("PoseStamped"), "POSE_STAMPED");
        assert_eq!(to_screaming_snake("Foo"), "FOO");
    }
}
