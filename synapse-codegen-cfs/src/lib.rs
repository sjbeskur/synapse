use std::{
    collections::{HashMap, HashSet},
    error::Error as StdError,
    fmt,
};

use synapse_parser::ast::{
    ArraySuffix, Attribute, BaseType, ConstDecl, FieldDef, Item, Literal, MessageDef, PacketKind,
    PrimitiveType, StructDef, SynFile, TypeExpr,
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
            cfs_module: "cfs_sys",
            tlm_header: "CFE_MSG_TelemetryHeader_t",
            cmd_header: "CFE_MSG_CommandHeader_t",
        }
    }
}

/// Error returned when a parsed Synapse file cannot be emitted safely.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodegenError {
    /// Optional fields parse today, but cFS ABI codegen has no representation for them yet.
    OptionalFieldUnsupported { container: String, field: String },
    /// Field defaults parse today, but cFS ABI codegen does not generate initializers yet.
    DefaultValueUnsupported { container: String, field: String },
    /// Enum fields parse today, but cFS ABI codegen has no explicit representation for them yet.
    EnumFieldUnsupported {
        container: String,
        field: String,
        ty: String,
    },
    /// The legacy `message` keyword is parsed for migration, but cFS codegen requires intent.
    LegacyMessageUnsupported { packet: String },
    /// cFS Software Bus command and telemetry packets require explicit message IDs.
    MissingMid { packet: String },
    /// Literal MIDs must be unique within one generated file.
    DuplicateMid {
        mid: String,
        first_packet: String,
        second_packet: String,
    },
}

impl fmt::Display for CodegenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CodegenError::OptionalFieldUnsupported { container, field } => write!(
                f,
                "optional field `{container}.{field}` is not supported by cFS codegen yet"
            ),
            CodegenError::DefaultValueUnsupported { container, field } => write!(
                f,
                "default value for field `{container}.{field}` is not supported by cFS codegen yet"
            ),
            CodegenError::EnumFieldUnsupported {
                container,
                field,
                ty,
            } => write!(
                f,
                "enum field `{container}.{field}` with type `{ty}` is not supported by cFS codegen yet"
            ),
            CodegenError::LegacyMessageUnsupported { packet } => write!(
                f,
                "legacy message `{packet}` is not supported by cFS codegen; use `command` or `telemetry`"
            ),
            CodegenError::MissingMid { packet } => {
                write!(f, "packet `{packet}` is missing required `@mid(...)`")
            }
            CodegenError::DuplicateMid {
                mid,
                first_packet,
                second_packet,
            } => write!(
                f,
                "duplicate MID `{mid}` used by packets `{first_packet}` and `{second_packet}`"
            ),
        }
    }
}

impl StdError for CodegenError {}

/// Generate a NASA cFS C header (`*_msg.h` + MID `#define`s) from a parsed Synapse file.
pub fn generate_c(file: &SynFile) -> String {
    try_generate_c(file).expect("parsed Synapse file is not supported by cFS C codegen")
}

/// Try to generate a NASA cFS C header (`*_msg.h` + MID `#define`s`) from a parsed Synapse file.
pub fn try_generate_c(file: &SynFile) -> Result<String, CodegenError> {
    validate_supported(file)?;
    let mut out = String::from(PREAMBLE);
    emit_c_imports(file, &mut out);
    emit_items(file, &mut out);
    Ok(out)
}

/// Generate `#[repr(C)]` Rust structs compatible with NASA cFS bindings.
///
/// `command` and `telemetry` packets become structs with the cFS header as the
/// first field, matching the C ABI layout. `struct` and `table` items remain
/// plain data structs. MID constants are emitted as `pub const`.
pub fn generate_rust(file: &SynFile, opts: &RustOptions) -> String {
    try_generate_rust(file, opts).expect("parsed Synapse file is not supported by cFS Rust codegen")
}

/// Try to generate `#[repr(C)]` Rust structs compatible with NASA cFS bindings.
pub fn try_generate_rust(file: &SynFile, opts: &RustOptions) -> Result<String, CodegenError> {
    validate_supported(file)?;
    let mut out = String::new();
    emit_rust_imports(file, &mut out);
    emit_rust_items(file, opts, &mut out);
    Ok(out)
}

// ── Item emission ─────────────────────────────────────────────────────────────

fn validate_supported(file: &SynFile) -> Result<(), CodegenError> {
    let enum_names = enum_names(file);
    let mut literal_mids = HashMap::new();
    for item in &file.items {
        match item {
            Item::Struct(s) | Item::Table(s) => validate_fields(&s.name, &s.fields, &enum_names)?,
            Item::Command(m) | Item::Telemetry(m) => {
                validate_packet(m, &mut literal_mids)?;
                validate_fields(&m.name, &m.fields, &enum_names)?
            }
            Item::Message(m) => {
                return Err(CodegenError::LegacyMessageUnsupported {
                    packet: m.name.clone(),
                });
            }
            Item::Namespace(_) | Item::Import(_) | Item::Const(_) | Item::Enum(_) => {}
        }
    }
    Ok(())
}

fn enum_names(file: &SynFile) -> HashSet<String> {
    file.items
        .iter()
        .filter_map(|item| match item {
            Item::Enum(e) => Some(e.name.clone()),
            _ => None,
        })
        .collect()
}

fn validate_packet(
    packet: &MessageDef,
    literal_mids: &mut HashMap<u64, String>,
) -> Result<(), CodegenError> {
    let Some(mid) = find_mid_attr(&packet.attrs) else {
        return Err(CodegenError::MissingMid {
            packet: packet.name.clone(),
        });
    };

    if let Some(value) = literal_to_u64(mid) {
        if let Some(first_packet) = literal_mids.insert(value, packet.name.clone()) {
            return Err(CodegenError::DuplicateMid {
                mid: literal_mid_str(mid),
                first_packet,
                second_packet: packet.name.clone(),
            });
        }
    }

    Ok(())
}

fn validate_fields(
    container: &str,
    fields: &[FieldDef],
    enum_names: &HashSet<String>,
) -> Result<(), CodegenError> {
    for field in fields {
        if field.optional {
            return Err(CodegenError::OptionalFieldUnsupported {
                container: container.to_string(),
                field: field.name.clone(),
            });
        }
        if field.default.is_some() {
            return Err(CodegenError::DefaultValueUnsupported {
                container: container.to_string(),
                field: field.name.clone(),
            });
        }
        if let BaseType::Ref(segments) = &field.ty.base {
            if segments
                .last()
                .is_some_and(|name| enum_names.contains(name.as_str()))
            {
                return Err(CodegenError::EnumFieldUnsupported {
                    container: container.to_string(),
                    field: field.name.clone(),
                    ty: segments.join("::"),
                });
            }
        }
    }
    Ok(())
}

fn emit_c_imports(file: &SynFile, out: &mut String) {
    let mut emitted = false;
    for item in &file.items {
        if let Item::Import(import) = item {
            out.push_str(&format!("#include \"{}\"\n", import_c_header(&import.path)));
            emitted = true;
        }
    }
    if emitted {
        out.push('\n');
    }
}

fn emit_rust_imports(file: &SynFile, out: &mut String) {
    let mut emitted = false;
    for item in &file.items {
        if let Item::Import(import) = item {
            out.push_str(&format!(
                "use crate::{};\n",
                import_rust_module(&import.path)
            ));
            emitted = true;
        }
    }
    if emitted {
        out.push('\n');
    }
}

fn emit_items(file: &SynFile, out: &mut String) {
    // First pass: emit #define MID lines for Software Bus packets with @mid
    let mut has_mids = false;
    for item in &file.items {
        if let Some(m) = packet_item(item) {
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
    if has_mids {
        out.push('\n');
    }

    // Second pass: emit const, struct, and message types
    let mut namespace = Vec::new();
    for item in &file.items {
        match item {
            Item::Namespace(ns) => namespace = ns.name.clone(),
            Item::Import(_) | Item::Enum(_) => {}
            Item::Const(c) => emit_const(out, c),
            Item::Struct(s) | Item::Table(s) => emit_struct(out, s, &namespace),
            Item::Command(m) | Item::Telemetry(m) | Item::Message(m) => {
                emit_message(out, m, &namespace)
            }
        }
    }
}

// ── Const ─────────────────────────────────────────────────────────────────────

fn emit_const(out: &mut String, c: &ConstDecl) {
    emit_doc_lines(out, &c.doc);
    let val = literal_str(&c.value);
    out.push_str(&format!("#define {}  {}\n\n", c.name, val));
}

// ── Struct (plain supporting type, no cFS header) ─────────────────────────────

fn emit_struct(out: &mut String, s: &StructDef, namespace: &[String]) {
    emit_doc_lines(out, &s.doc);
    out.push_str("typedef struct {\n");
    for f in &s.fields {
        emit_c_field(out, f, namespace);
    }
    out.push_str(&format!("}} {};\n\n", c_decl_type_name(&s.name, namespace)));
}

// ── Message ───────────────────────────────────────────────────────────────────

fn emit_message(out: &mut String, m: &MessageDef, namespace: &[String]) {
    let header_type = if packet_is_command(m) {
        "CFE_MSG_CommandHeader_t"
    } else {
        "CFE_MSG_TelemetryHeader_t"
    };

    emit_doc_lines(out, &m.doc);

    out.push_str(&format!("typedef struct {{\n"));
    out.push_str(&format!("    {} Header;\n", header_type));
    for f in &m.fields {
        emit_c_field(out, f, namespace);
    }
    out.push_str(&format!("}} {};\n\n", c_decl_type_name(&m.name, namespace)));
}

// ── Rust emission ─────────────────────────────────────────────────────────────

fn emit_rust_items(file: &SynFile, opts: &RustOptions, out: &mut String) {
    // First pass: MID consts for Software Bus packets with @mid
    let mut has_mids = false;
    for item in &file.items {
        if let Some(m) = packet_item(item) {
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
    if has_mids {
        out.push('\n');
    }

    // Second pass: types
    for item in &file.items {
        match item {
            Item::Namespace(_) | Item::Import(_) | Item::Enum(_) => {}
            Item::Const(c) => emit_rust_const(out, c),
            Item::Struct(s) | Item::Table(s) => emit_rust_struct(out, s),
            Item::Command(m) | Item::Telemetry(m) | Item::Message(m) => {
                emit_rust_message(out, m, opts)
            }
        }
    }
}

fn emit_rust_const(out: &mut String, c: &ConstDecl) {
    emit_doc_lines(out, &c.doc);
    let val = rust_literal_str(&c.value);
    let ty = rust_field_type_str(&c.ty);
    out.push_str(&format!("pub const {}: {} = {};\n\n", c.name, ty, val));
}

fn emit_rust_struct(out: &mut String, s: &StructDef) {
    emit_doc_lines(out, &s.doc);
    out.push_str("#[repr(C)]\n");
    out.push_str(&format!("pub struct {} {{\n", s.name));
    for f in &s.fields {
        emit_indented_doc_lines(out, &f.doc);
        out.push_str(&format!(
            "    pub {}: {},\n",
            f.name,
            rust_field_type_str(&f.ty)
        ));
    }
    out.push_str("}\n\n");
}

fn emit_rust_message(out: &mut String, m: &MessageDef, opts: &RustOptions) {
    let header_type = if packet_is_command(m) {
        opts.cmd_header
    } else {
        opts.tlm_header
    };
    let qualified = if opts.cfs_module.is_empty() {
        header_type.to_string()
    } else {
        format!("{}::{}", opts.cfs_module, header_type)
    };

    emit_doc_lines(out, &m.doc);

    out.push_str("#[repr(C)]\n");
    out.push_str(&format!("pub struct {} {{\n", m.name));
    out.push_str(&format!("    pub cfs_header: {},\n", qualified));
    for f in &m.fields {
        emit_indented_doc_lines(out, &f.doc);
        let ty = rust_field_type_str(&f.ty);
        out.push_str(&format!("    pub {}: {},\n", f.name, ty));
    }
    out.push_str("}\n\n");
}

fn rust_field_type_str(ty: &TypeExpr) -> String {
    if ty.base == BaseType::String {
        return match &ty.array {
            None | Some(ArraySuffix::Dynamic) => "*const u8".to_string(),
            Some(ArraySuffix::Fixed(n)) | Some(ArraySuffix::Bounded(n)) => {
                format!("[u8; {}]", n)
            }
        };
    }

    let base = rust_base_type_str(&ty.base);
    match &ty.array {
        None => base,
        Some(ArraySuffix::Fixed(n)) => format!("[{}; {}]", base, n),
        // Dynamic/bounded: use a raw slice pointer — no alloc in cFS context
        Some(ArraySuffix::Dynamic) => format!("*const {}", base),
        Some(ArraySuffix::Bounded(n)) => format!("*const {}  /* max {} */", base, n),
    }
}

fn rust_base_type_str(base: &BaseType) -> String {
    match base {
        BaseType::String => "*const u8".to_string(),
        BaseType::Primitive(p) => rust_primitive_str(*p).to_string(),
        BaseType::Ref(segments) => segments.join("::"),
    }
}

fn rust_primitive_str(p: PrimitiveType) -> &'static str {
    match p {
        PrimitiveType::F32 => "f32",
        PrimitiveType::F64 => "f64",
        PrimitiveType::I8 => "i8",
        PrimitiveType::I16 => "i16",
        PrimitiveType::I32 => "i32",
        PrimitiveType::I64 => "i64",
        PrimitiveType::U8 => "u8",
        PrimitiveType::U16 => "u16",
        PrimitiveType::U32 => "u32",
        PrimitiveType::U64 => "u64",
        PrimitiveType::Bool => "bool",
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
        Literal::Hex(n) => format!("0x{:X}", n),
        Literal::Int(n) => n.to_string(),
        Literal::Bool(b) => b.to_string(),
        Literal::Float(f) => {
            let s = format!("{}", f);
            if s.contains('.') || s.contains('e') {
                s
            } else {
                format!("{}.0", s)
            }
        }
        Literal::Str(s) => format!("{:?}", s),
        Literal::Ident(segments) => segments.join("::"),
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Returns the `@mid` attribute value, if present.
fn find_mid_attr(attrs: &[Attribute]) -> Option<&Literal> {
    attrs.iter().find(|a| a.name == "mid").map(|a| &a.value)
}

fn packet_item(item: &Item) -> Option<&MessageDef> {
    match item {
        Item::Command(m) | Item::Telemetry(m) => Some(m),
        _ => None,
    }
}

fn packet_is_command(m: &MessageDef) -> bool {
    match m.kind {
        PacketKind::Command => true,
        PacketKind::Telemetry | PacketKind::Message => false,
    }
}

fn literal_to_u64(lit: &Literal) -> Option<u64> {
    match lit {
        Literal::Hex(n) => Some(*n),
        Literal::Int(n) if *n >= 0 => Some(*n as u64),
        _ => None,
    }
}

fn emit_doc_lines(out: &mut String, doc: &[String]) {
    for line in doc {
        if line.is_empty() {
            out.push_str("///\n");
        } else {
            out.push_str(&format!("/// {line}\n"));
        }
    }
}

fn emit_indented_doc_lines(out: &mut String, doc: &[String]) {
    for line in doc {
        if line.is_empty() {
            out.push_str("    ///\n");
        } else {
            out.push_str(&format!("    /// {line}\n"));
        }
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
            if s.contains('.') || s.contains('e') {
                s
            } else {
                format!("{}.0", s)
            }
        }
        Literal::Int(n) => n.to_string(),
        Literal::Hex(n) => format!("0x{:X}U", n),
        Literal::Bool(b) => {
            if *b {
                "1".to_string()
            } else {
                "0".to_string()
            }
        }
        Literal::Str(s) => format!("{:?}", s),
        Literal::Ident(segments) => segments.join("::"),
    }
}

fn non_fixed_type_str(ty: &TypeExpr, namespace: &[String]) -> String {
    if ty.base == BaseType::String {
        return match &ty.array {
            None | Some(ArraySuffix::Dynamic) => "const char*".to_string(),
            Some(ArraySuffix::Fixed(_)) => unreachable!("handled by emit_c_field"),
            Some(ArraySuffix::Bounded(n)) => format!("char[{}]", n),
        };
    }

    let base = base_type_str(&ty.base, namespace);
    match &ty.array {
        None => base,
        Some(ArraySuffix::Fixed(_)) => unreachable!("handled by caller"),
        Some(ArraySuffix::Dynamic) => format!("CFE_Span_t /* {} */", base),
        Some(ArraySuffix::Bounded(n)) => format!("CFE_Span_t /* {} max {} */", base, n),
    }
}

fn base_type_str(base: &BaseType, namespace: &[String]) -> String {
    match base {
        BaseType::String => "const char*".to_string(),
        BaseType::Primitive(p) => primitive_str(*p).to_string(),
        BaseType::Ref(segments) => c_ref_type_name(segments, namespace),
    }
}

fn emit_c_field(out: &mut String, f: &synapse_parser::ast::FieldDef, namespace: &[String]) {
    emit_indented_doc_lines(out, &f.doc);
    match (&f.ty.base, &f.ty.array) {
        (BaseType::String, Some(ArraySuffix::Fixed(n) | ArraySuffix::Bounded(n))) => {
            out.push_str(&format!("    char {}[{}];\n", f.name, n));
        }
        (_, Some(ArraySuffix::Fixed(n))) => {
            out.push_str(&format!(
                "    {} {}[{}];\n",
                base_type_str(&f.ty.base, namespace),
                f.name,
                n
            ));
        }
        _ => {
            out.push_str(&format!(
                "    {} {};\n",
                non_fixed_type_str(&f.ty, namespace),
                f.name
            ));
        }
    }
}

fn c_decl_type_name(name: &str, namespace: &[String]) -> String {
    let mut segments = namespace.to_vec();
    segments.push(name.to_string());
    format!("{}_t", segments.join("_"))
}

fn c_ref_type_name(segments: &[String], namespace: &[String]) -> String {
    let resolved = if segments.len() == 1 && !namespace.is_empty() {
        let mut resolved = namespace.to_vec();
        resolved.push(segments[0].clone());
        resolved
    } else {
        segments.to_vec()
    };
    if resolved.is_empty() {
        return "_t".to_string();
    }
    format!("{}_t", resolved.join("_"))
}

fn import_c_header(path: &str) -> String {
    replace_extension(path, "h")
}

fn import_rust_module(path: &str) -> String {
    let header = path.rsplit('/').next().unwrap_or(path);
    replace_extension(header, "")
}

fn replace_extension(path: &str, ext: &str) -> String {
    match path.rsplit_once('.') {
        Some((stem, _)) if ext.is_empty() => stem.to_string(),
        Some((stem, _)) => format!("{stem}.{ext}"),
        None if ext.is_empty() => path.to_string(),
        None => format!("{path}.{ext}"),
    }
}

fn primitive_str(p: PrimitiveType) -> &'static str {
    match p {
        PrimitiveType::F32 => "float",
        PrimitiveType::F64 => "double",
        PrimitiveType::I8 => "int8_t",
        PrimitiveType::I16 => "int16_t",
        PrimitiveType::I32 => "int32_t",
        PrimitiveType::I64 => "int64_t",
        PrimitiveType::U8 => "uint8_t",
        PrimitiveType::U16 => "uint16_t",
        PrimitiveType::U32 => "uint32_t",
        PrimitiveType::U64 => "uint64_t",
        PrimitiveType::Bool => "bool",
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

    fn codegen(src: &str) -> String {
        generate_c(&parse(src).unwrap())
    }

    #[test]
    fn telemetry_with_hex_mid() {
        let out = codegen("@mid(0x0801)\ntelemetry NavTlm { x: f64  y: f64 }");
        assert!(out.contains("#define NAV_TLM_MID  0x0801U"));
        assert!(out.contains("CFE_MSG_TelemetryHeader_t Header;"));
        assert!(out.contains("typedef struct {"));
        assert!(out.contains("} NavTlm_t;"));
        assert!(out.contains("    double x;"));
        assert!(out.contains("    double y;"));
    }

    #[test]
    fn command_uses_declared_packet_kind() {
        let out = codegen("@mid(0x0801)\ncommand NavCmd { seq: u16 }");
        assert!(out.contains("#define NAV_CMD_MID  0x0801U"));
        assert!(out.contains("CFE_MSG_CommandHeader_t Header;"));
        assert!(out.contains("} NavCmd_t;"));
    }

    #[test]
    fn command_uses_command_header() {
        let out = codegen("@mid(0x1880)\ncommand SetMode { mode: u8 }");
        assert!(out.contains("#define SET_MODE_MID  0x1880U"));
        assert!(out.contains("CFE_MSG_CommandHeader_t Header;"));
        assert!(!out.contains("CFE_MSG_TelemetryHeader_t Header;"));
    }

    #[test]
    fn telemetry_uses_telemetry_header() {
        let out = codegen("@mid(0x0801)\ntelemetry NavState { x: f64 }");
        assert!(out.contains("#define NAV_STATE_MID  0x0801U"));
        assert!(out.contains("CFE_MSG_TelemetryHeader_t Header;"));
        assert!(!out.contains("CFE_MSG_CommandHeader_t Header;"));
    }

    #[test]
    fn table_is_plain_data_without_bus_header() {
        let out = codegen("table NavConfig { max_speed: f64  enabled: bool }");
        assert!(out.contains("} NavConfig_t;"));
        assert!(out.contains("    double max_speed;"));
        assert!(!out.contains("CFE_MSG_CommandHeader_t Header;"));
        assert!(!out.contains("CFE_MSG_TelemetryHeader_t Header;"));
    }

    #[test]
    fn c_rejects_legacy_message() {
        let file = parse("message Bare { x: f32 }").unwrap();
        let err = try_generate_c(&file).unwrap_err();
        assert_eq!(
            err,
            CodegenError::LegacyMessageUnsupported {
                packet: "Bare".to_string(),
            }
        );
        assert_eq!(
            err.to_string(),
            "legacy message `Bare` is not supported by cFS codegen; use `command` or `telemetry`"
        );
    }

    #[test]
    fn c_rejects_command_without_mid() {
        let file = parse("command SetMode { mode: u8 }").unwrap();
        let err = try_generate_c(&file).unwrap_err();
        assert_eq!(
            err,
            CodegenError::MissingMid {
                packet: "SetMode".to_string(),
            }
        );
        assert_eq!(
            err.to_string(),
            "packet `SetMode` is missing required `@mid(...)`"
        );
    }

    #[test]
    fn c_rejects_duplicate_literal_mids() {
        let file =
            parse("@mid(0x1880)\ncommand A { x: u8 }\n@mid(0x1880)\ncommand B { x: u8 }").unwrap();
        let err = try_generate_c(&file).unwrap_err();
        assert_eq!(
            err,
            CodegenError::DuplicateMid {
                mid: "0x1880U".to_string(),
                first_packet: "A".to_string(),
                second_packet: "B".to_string(),
            }
        );
        assert_eq!(
            err.to_string(),
            "duplicate MID `0x1880U` used by packets `A` and `B`"
        );
    }

    #[test]
    fn c_rejects_optional_fields() {
        let file = parse("@mid(0x0801)\ntelemetry Status { error_code?: u32 }").unwrap();
        let err = try_generate_c(&file).unwrap_err();
        assert_eq!(
            err,
            CodegenError::OptionalFieldUnsupported {
                container: "Status".to_string(),
                field: "error_code".to_string(),
            }
        );
        assert_eq!(
            err.to_string(),
            "optional field `Status.error_code` is not supported by cFS codegen yet"
        );
    }

    #[test]
    fn c_rejects_default_values() {
        let file = parse("table Config { exposure_us: u32 = 10000 }").unwrap();
        let err = try_generate_c(&file).unwrap_err();
        assert_eq!(
            err,
            CodegenError::DefaultValueUnsupported {
                container: "Config".to_string(),
                field: "exposure_us".to_string(),
            }
        );
        assert_eq!(
            err.to_string(),
            "default value for field `Config.exposure_us` is not supported by cFS codegen yet"
        );
    }

    #[test]
    fn c_rejects_enum_fields() {
        let file = parse(
            "enum CameraMode { Idle = 0 Streaming = 1 }\n@mid(0x0801)\ntelemetry Status { mode: CameraMode }",
        )
        .unwrap();
        let err = try_generate_c(&file).unwrap_err();
        assert_eq!(
            err,
            CodegenError::EnumFieldUnsupported {
                container: "Status".to_string(),
                field: "mode".to_string(),
                ty: "CameraMode".to_string(),
            }
        );
        assert_eq!(
            err.to_string(),
            "enum field `Status.mode` with type `CameraMode` is not supported by cFS codegen yet"
        );
    }

    #[test]
    fn const_emits_define() {
        let out = codegen("const NAV_TLM_MID: u16 = 0x0801");
        assert!(out.contains("#define NAV_TLM_MID  0x801U"));
    }

    #[test]
    fn fixed_array_field() {
        let out = codegen("@mid(0x0802)\ntelemetry Imu { covariance: f64[9] }");
        assert!(out.contains("    double covariance[9];"));
    }

    #[test]
    fn c_refs_use_declared_typedef_names() {
        let out = codegen("struct Point { x: f64 }\n@mid(0x0801)\ntelemetry Pose { point: Point }");
        assert!(out.contains("} Point_t;"));
        assert!(out.contains("    Point_t point;"));
    }

    #[test]
    fn c_qualified_refs_use_declared_typedef_names() {
        let out = codegen("@mid(0x0801)\ntelemetry Stamped { header: std_msgs::Header }");
        assert!(out.contains("    std_msgs_Header_t header;"));
    }

    #[test]
    fn c_bounded_string_uses_inline_storage() {
        let out = codegen("struct Label { name: string[<=64] }");
        assert!(out.contains("    char name[64];"));
    }

    #[test]
    fn c_imports_emit_header_includes() {
        let out = codegen(r#"import "std_msgs.syn""#);
        assert!(out.contains("#include \"std_msgs.h\""));
    }

    #[test]
    fn c_doc_comments_emit_for_declarations_and_fields() {
        let out = codegen("/// A point\nstruct Point {\n/// X axis\nx: f64\n}");
        assert!(out.contains("/// A point\ntypedef struct {"));
        assert!(out.contains("    /// X axis\n    double x;"));
    }

    // ── Rust codegen ─────────────────────────────────────────

    fn rust_codegen(src: &str) -> String {
        generate_rust(&parse(src).unwrap(), &RustOptions::default())
    }

    #[test]
    fn rust_tlm_struct() {
        let out = rust_codegen("@mid(0x0801)\ntelemetry NavTlm { x: f64  y: f64 }");
        assert!(out.contains("pub const NAV_TLM_MID: u16 = 0x0801;"));
        assert!(out.contains("#[repr(C)]"));
        assert!(out.contains("pub struct NavTlm {"));
        assert!(out.contains("    pub cfs_header: cfs_sys::CFE_MSG_TelemetryHeader_t,"));
        assert!(out.contains("    pub x: f64,"));
        assert!(out.contains("    pub y: f64,"));
    }

    #[test]
    fn rust_cmd_struct() {
        let out = rust_codegen("@mid(0x1880)\ncommand NavCmd { seq: u16 }");
        assert!(out.contains("pub const NAV_CMD_MID: u16 = 0x1880;"));
        assert!(out.contains("    pub cfs_header: cfs_sys::CFE_MSG_CommandHeader_t,"));
    }

    #[test]
    fn rust_command_uses_command_header() {
        let out = rust_codegen("@mid(0x0801)\ncommand SetMode { mode: u8 }");
        assert!(out.contains("pub const SET_MODE_MID: u16 = 0x0801;"));
        assert!(out.contains("    pub cfs_header: cfs_sys::CFE_MSG_CommandHeader_t,"));
        assert!(!out.contains("CFE_MSG_TelemetryHeader_t"));
    }

    #[test]
    fn rust_telemetry_uses_telemetry_header() {
        let out = rust_codegen("@mid(0x1880)\ntelemetry NavState { x: f64 }");
        assert!(out.contains("pub const NAV_STATE_MID: u16 = 0x1880;"));
        assert!(out.contains("    pub cfs_header: cfs_sys::CFE_MSG_TelemetryHeader_t,"));
        assert!(!out.contains("CFE_MSG_CommandHeader_t"));
    }

    #[test]
    fn rust_table_is_plain_data_without_bus_header() {
        let out = rust_codegen("table NavConfig { max_speed: f64  enabled: bool }");
        assert!(out.contains("pub struct NavConfig {"));
        assert!(out.contains("    pub max_speed: f64,"));
        assert!(!out.contains("cfs_header"));
    }

    #[test]
    fn rust_fixed_array() {
        let out = rust_codegen("@mid(0x0802)\ntelemetry Imu { covariance: f64[9] }");
        assert!(out.contains("    pub covariance: [f64; 9],"));
    }

    #[test]
    fn rust_custom_module() {
        let opts = RustOptions {
            cfs_module: "my_cfs",
            ..Default::default()
        };
        let out = generate_rust(
            &parse("@mid(0x0801)\ntelemetry T { x: f32 }").unwrap(),
            &opts,
        );
        assert!(out.contains("my_cfs::CFE_MSG_TelemetryHeader_t"));
    }

    #[test]
    fn rust_bare_module() {
        let opts = RustOptions {
            cfs_module: "",
            ..Default::default()
        };
        let out = generate_rust(
            &parse("@mid(0x0801)\ntelemetry T { x: f32 }").unwrap(),
            &opts,
        );
        assert!(out.contains("    pub cfs_header: CFE_MSG_TelemetryHeader_t,"));
        assert!(!out.contains("::CFE_MSG_TelemetryHeader_t"));
    }

    #[test]
    fn rust_message_can_have_payload_header_field() {
        let out = rust_codegen("@mid(0x0801)\ntelemetry Stamped { header: std_msgs::Header }");
        assert!(out.contains("    pub cfs_header: cfs_sys::CFE_MSG_TelemetryHeader_t,"));
        assert!(out.contains("    pub header: std_msgs::Header,"));
    }

    #[test]
    fn rust_rejects_legacy_message() {
        let file = parse("@mid(0x0801)\nmessage Bare { x: f32 }").unwrap();
        let err = try_generate_rust(&file, &RustOptions::default()).unwrap_err();
        assert_eq!(
            err,
            CodegenError::LegacyMessageUnsupported {
                packet: "Bare".to_string(),
            }
        );
    }

    #[test]
    fn rust_rejects_telemetry_without_mid() {
        let file = parse("telemetry Status { x: f32 }").unwrap();
        let err = try_generate_rust(&file, &RustOptions::default()).unwrap_err();
        assert_eq!(
            err,
            CodegenError::MissingMid {
                packet: "Status".to_string(),
            }
        );
    }

    #[test]
    fn rust_rejects_optional_fields() {
        let file = parse("struct Status { error_code?: u32 }").unwrap();
        let err = try_generate_rust(&file, &RustOptions::default()).unwrap_err();
        assert_eq!(
            err,
            CodegenError::OptionalFieldUnsupported {
                container: "Status".to_string(),
                field: "error_code".to_string(),
            }
        );
    }

    #[test]
    fn rust_rejects_default_values() {
        let file = parse("struct Config { gain: f32 = 1.0 }").unwrap();
        let err = try_generate_rust(&file, &RustOptions::default()).unwrap_err();
        assert_eq!(
            err,
            CodegenError::DefaultValueUnsupported {
                container: "Config".to_string(),
                field: "gain".to_string(),
            }
        );
    }

    #[test]
    fn rust_rejects_enum_fields() {
        let file = parse("enum CameraMode { Idle Streaming }\nstruct Status { mode: CameraMode }")
            .unwrap();
        let err = try_generate_rust(&file, &RustOptions::default()).unwrap_err();
        assert_eq!(
            err,
            CodegenError::EnumFieldUnsupported {
                container: "Status".to_string(),
                field: "mode".to_string(),
                ty: "CameraMode".to_string(),
            }
        );
    }

    #[test]
    fn rust_const_uses_declared_type() {
        let out = rust_codegen("const PI: f64 = 3.14\nconst ENABLED: bool = true");
        assert!(out.contains("pub const PI: f64 = 3.14;"));
        assert!(out.contains("pub const ENABLED: bool = true;"));
    }

    #[test]
    fn rust_bounded_string_uses_inline_storage() {
        let out = rust_codegen("struct Label { name: string[<=64] }");
        assert!(out.contains("    pub name: [u8; 64],"));
    }

    #[test]
    fn rust_imports_emit_crate_uses() {
        let out = rust_codegen(r#"import "std_msgs.syn""#);
        assert!(out.contains("use crate::std_msgs;"));
    }

    #[test]
    fn rust_doc_comments_emit_for_declarations_and_fields() {
        let out = rust_codegen("/// A point\nstruct Point {\n/// X axis\nx: f64\n}");
        assert!(out.contains("/// A point\n#[repr(C)]\npub struct Point {"));
        assert!(out.contains("    /// X axis\n    pub x: f64,"));
    }

    #[test]
    fn screaming_snake_conversion() {
        assert_eq!(to_screaming_snake("NavTelemetry"), "NAV_TELEMETRY");
        assert_eq!(to_screaming_snake("PoseStamped"), "POSE_STAMPED");
        assert_eq!(to_screaming_snake("Foo"), "FOO");
    }
}
