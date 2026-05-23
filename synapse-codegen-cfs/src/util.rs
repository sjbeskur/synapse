use std::collections::HashMap;

use synapse_parser::ast::{
    ArraySuffix, Attribute, BaseType, EnumDef, Item, Literal, MessageDef, PacketKind,
    PrimitiveType, SynFile, TypeExpr,
};

use crate::constants::{ConstContext, resolve_ident_to_u64};

pub(crate) fn file_namespace(file: &SynFile) -> Vec<String> {
    file.items
        .iter()
        .find_map(|item| match item {
            Item::Namespace(ns) => Some(ns.name.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

pub(crate) fn enum_defs(file: &SynFile) -> HashMap<String, &EnumDef> {
    file.items
        .iter()
        .filter_map(|item| match item {
            Item::Enum(e) => Some((e.name.clone(), e)),
            _ => None,
        })
        .collect()
}

/// Returns the `@mid` attribute value, if present.
pub(crate) fn find_mid_attr(attrs: &[Attribute]) -> Option<&Literal> {
    attrs.iter().find(|a| a.name == "mid").map(|a| &a.value)
}

/// Returns the `@cc` attribute value, if present.
pub(crate) fn find_cc_attr(attrs: &[Attribute]) -> Option<&Literal> {
    attrs.iter().find(|a| a.name == "cc").map(|a| &a.value)
}

pub(crate) fn packet_item(item: &Item) -> Option<&MessageDef> {
    match item {
        Item::Command(m) | Item::Telemetry(m) => Some(m),
        _ => None,
    }
}

pub(crate) fn packet_is_command(m: &MessageDef) -> bool {
    match m.kind {
        PacketKind::Command => true,
        PacketKind::Telemetry | PacketKind::Message => false,
    }
}

pub(crate) fn type_expr_display(ty: &TypeExpr) -> String {
    let mut out = base_type_display(&ty.base);
    match &ty.array {
        None => {}
        Some(ArraySuffix::Dynamic) => out.push_str("[]"),
        Some(ArraySuffix::Fixed(n)) => out.push_str(&format!("[{n}]")),
        Some(ArraySuffix::Bounded(n)) => out.push_str(&format!("[<={n}]")),
    }
    out
}

fn base_type_display(base: &BaseType) -> String {
    match base {
        BaseType::String => "string".to_string(),
        BaseType::Primitive(p) => primitive_name(*p).to_string(),
        BaseType::Ref(segments) => segments.join("::"),
    }
}

pub(crate) fn primitive_name(p: PrimitiveType) -> &'static str {
    const NAMES: &[(PrimitiveType, &str)] = &[
        (PrimitiveType::F32, "f32"),
        (PrimitiveType::F64, "f64"),
        (PrimitiveType::I8, "i8"),
        (PrimitiveType::I16, "i16"),
        (PrimitiveType::I32, "i32"),
        (PrimitiveType::I64, "i64"),
        (PrimitiveType::U8, "u8"),
        (PrimitiveType::U16, "u16"),
        (PrimitiveType::U32, "u32"),
        (PrimitiveType::U64, "u64"),
        (PrimitiveType::Bool, "bool"),
        (PrimitiveType::Bytes, "bytes"),
    ];

    NAMES
        .iter()
        .find_map(|(ty, name)| (*ty == p).then_some(*name))
        .expect("all primitive types have Synapse names")
}

pub(crate) fn emit_doc_lines(out: &mut String, doc: &[String]) {
    for line in doc {
        if line.is_empty() {
            out.push_str("///\n");
        } else {
            out.push_str(&format!("/// {line}\n"));
        }
    }
}

pub(crate) fn emit_indented_doc_lines(out: &mut String, doc: &[String]) {
    for line in doc {
        if line.is_empty() {
            out.push_str("    ///\n");
        } else {
            out.push_str(&format!("    /// {line}\n"));
        }
    }
}

/// Format a MID literal for a C `#define` line.
pub(crate) fn literal_mid_str(lit: &Literal, constants: &ConstContext<'_>) -> String {
    match lit {
        Literal::Hex(n) => format!("0x{:04X}U", n),
        Literal::Int(n) => format!("{}U", n),
        Literal::Ident(segs) if constants.is_local_bare_ident(segs) => segs.join("::"),
        Literal::Ident(segs) => resolve_ident_to_u64(segs, constants)
            .map(|value| format!("0x{:04X}U", value))
            .unwrap_or_else(|| segs.join("::")),
        other => literal_str(other),
    }
}

/// Format a command-code literal for a C `#define` line.
pub(crate) fn literal_cc_str(lit: &Literal, constants: &ConstContext<'_>) -> String {
    match lit {
        Literal::Hex(n) => format!("0x{:X}U", n),
        Literal::Int(n) => format!("{}U", n),
        Literal::Ident(segs) if constants.is_local_bare_ident(segs) => segs.join("::"),
        Literal::Ident(segs) => resolve_ident_to_u64(segs, constants)
            .map(|value| format!("{}U", value))
            .unwrap_or_else(|| segs.join("::")),
        other => literal_str(other),
    }
}

pub(crate) fn literal_str(lit: &Literal) -> String {
    match lit {
        Literal::Float(_) | Literal::Int(_) | Literal::Hex(_) | Literal::Bool(_) => {
            c_scalar_literal_str(lit)
        }
        Literal::Str(s) => format!("{:?}", s),
        Literal::Ident(segments) => segments.join("::"),
    }
}

fn c_scalar_literal_str(lit: &Literal) -> String {
    if let Literal::Float(f) = lit {
        return c_float_literal_str(*f);
    }
    if let Literal::Int(n) = lit {
        return n.to_string();
    }
    if let Literal::Hex(n) = lit {
        return format!("0x{:X}U", n);
    }
    if let Literal::Bool(b) = lit {
        return c_bool_literal_str(*b);
    }
    unreachable!("non-scalar literal passed to c_scalar_literal_str")
}

fn c_float_literal_str(value: f64) -> String {
    let s = format!("{}", value);
    if s.contains('.') || s.contains('e') {
        s
    } else {
        format!("{}.0", s)
    }
}

fn c_bool_literal_str(value: bool) -> String {
    if value {
        "1".to_string()
    } else {
        "0".to_string()
    }
}

pub(crate) fn typed_literal_str(lit: &Literal, ty: &TypeExpr) -> String {
    match (lit, &ty.base) {
        (Literal::Hex(n), BaseType::Primitive(p)) => c_hex_str(*n, *p),
        _ => literal_str(lit),
    }
}

fn c_hex_str(value: u64, ty: PrimitiveType) -> String {
    match ty {
        PrimitiveType::U8 | PrimitiveType::I8 => format!("0x{:02X}U", value),
        PrimitiveType::U16 | PrimitiveType::I16 => format!("0x{:04X}U", value),
        PrimitiveType::U32 | PrimitiveType::I32 => format!("0x{:08X}U", value),
        PrimitiveType::U64 | PrimitiveType::I64 => format!("0x{:016X}U", value),
        PrimitiveType::F32 | PrimitiveType::F64 | PrimitiveType::Bool | PrimitiveType::Bytes => {
            format!("0x{:X}U", value)
        }
    }
}

pub(crate) fn import_c_header(path: &str) -> String {
    replace_extension(path, "h")
}

pub(crate) fn import_rust_module(path: &str) -> String {
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

pub(crate) fn to_screaming_snake(name: &str) -> String {
    let mut out = String::new();
    for (i, ch) in name.chars().enumerate() {
        if ch.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(ch.to_ascii_uppercase());
    }
    out
}
