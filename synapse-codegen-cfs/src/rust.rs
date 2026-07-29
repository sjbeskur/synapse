use synapse_parser::ast::{
    ArraySuffix, BaseType, ConstDecl, EnumDef, Item, Literal, MessageDef, PrimitiveType, StructDef,
    SynFile, TypeExpr,
};

use crate::{
    constants::{ConstContext, const_context, resolve_ident_to_u64},
    error::CodegenError,
    types::{GENERATED_BANNER, ResolvedConstants, RustOptions},
    util::{
        emit_doc_lines, emit_indented_doc_lines, find_cc_attr, import_rust_module,
        packet_is_command, to_screaming_snake,
    },
    validate::validate_supported,
};

/// Generate `#[repr(C)]` Rust structs compatible with NASA cFS bindings.
///
/// `command` and `telemetry` packets become structs with the cFS header as the
/// first field, matching the C ABI layout. `struct` and `table` items remain
/// plain data structs.
pub fn generate_rust(file: &SynFile, opts: &RustOptions) -> String {
    try_generate_rust(file, opts).expect("parsed Synapse file is not supported by cFS Rust codegen")
}

/// Try to generate `#[repr(C)]` Rust structs compatible with NASA cFS bindings.
pub fn try_generate_rust(file: &SynFile, opts: &RustOptions) -> Result<String, CodegenError> {
    try_generate_rust_with_constants(file, opts, &ResolvedConstants::new())
}

/// Try to generate Rust bindings with additional imported constants available for attributes.
pub fn try_generate_rust_with_constants(
    file: &SynFile,
    opts: &RustOptions,
    imported_constants: &ResolvedConstants,
) -> Result<String, CodegenError> {
    let constants = const_context(file, imported_constants);
    validate_supported(file, &constants)?;
    let mut out = format!("// {GENERATED_BANNER}\n\n");
    emit_rust_imports(file, &mut out);
    emit_rust_items(file, opts, &mut out, &constants);
    Ok(out)
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

fn emit_rust_items(
    file: &SynFile,
    opts: &RustOptions,
    out: &mut String,
    constants: &ConstContext<'_>,
) {
    emit_rust_command_code_consts(file, out, constants);
    emit_rust_types(file, opts, out);
}

fn emit_rust_command_code_consts(file: &SynFile, out: &mut String, constants: &ConstContext<'_>) {
    let mut has_ccs = false;
    for item in &file.items {
        if let Item::Command(m) = item {
            if let Some(cc) = find_cc_attr(&m.attrs) {
                if !has_ccs {
                    out.push_str("// Command Codes\n");
                    has_ccs = true;
                }
                let const_name = format!("{}_CC", to_screaming_snake(&m.name));
                let val = rust_cc_str(cc, constants);
                out.push_str(&format!("pub const {}: u16 = {};\n", const_name, val));
            }
        }
    }
    if has_ccs {
        out.push('\n');
    }
}

fn emit_rust_types(file: &SynFile, opts: &RustOptions, out: &mut String) {
    for item in &file.items {
        if emit_rust_named_item(out, item) {
            continue;
        }
        if let Item::Command(m) | Item::Telemetry(m) | Item::Message(m) = item {
            emit_rust_message(out, m, opts);
        }
    }
}

fn emit_rust_named_item(out: &mut String, item: &Item) -> bool {
    match item {
        Item::Const(c) => emit_rust_const(out, c),
        Item::Enum(e) => emit_rust_enum(out, e),
        Item::Struct(s) | Item::Table(s) => emit_rust_struct(out, s),
        _ => return false,
    }
    true
}

fn emit_rust_const(out: &mut String, c: &ConstDecl) {
    emit_doc_lines(out, &c.doc);
    let val = rust_typed_literal_str(&c.value, &c.ty);
    let ty = rust_field_type_str(&c.ty);
    out.push_str(&format!("pub const {}: {} = {};\n\n", c.name, ty, val));
}

fn emit_rust_enum(out: &mut String, e: &EnumDef) {
    let Some(repr) = e.repr else {
        return;
    };

    emit_doc_lines(out, &e.doc);
    out.push_str(&format!(
        "pub type {} = {};\n",
        e.name,
        rust_primitive_str(repr)
    ));

    let enum_prefix = to_screaming_snake(&e.name);
    for variant in &e.variants {
        emit_doc_lines(out, &variant.doc);
        let value = variant
            .value
            .expect("represented enum variants validated before emission");
        out.push_str(&format!(
            "pub const {}_{}: {} = {};\n",
            enum_prefix,
            to_screaming_snake(&variant.name),
            e.name,
            value
        ));
    }
    out.push('\n');
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
        return rust_string_type_str(&ty.array);
    }

    let base = rust_base_type_str(&ty.base);
    rust_array_type_str(base, &ty.array)
}

fn rust_string_type_str(array: &Option<ArraySuffix>) -> String {
    match array {
        None | Some(ArraySuffix::Dynamic) => "*const u8".to_string(),
        Some(ArraySuffix::Fixed(n)) | Some(ArraySuffix::Bounded(n)) => {
            format!("[u8; {}]", n)
        }
    }
}

fn rust_array_type_str(base: String, array: &Option<ArraySuffix>) -> String {
    match array {
        None => base,
        Some(ArraySuffix::Fixed(n)) => format!("[{}; {}]", base, n),
        // Dynamic/bounded: use a raw slice pointer; no alloc in cFS context.
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
    const RUST_TYPES: &[(PrimitiveType, &str)] = &[
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
        (PrimitiveType::Bytes, "*const u8"),
    ];

    RUST_TYPES
        .iter()
        .find_map(|(ty, name)| (*ty == p).then_some(*name))
        .expect("all primitive types have Rust names")
}

fn rust_cc_str(lit: &Literal, constants: &ConstContext<'_>) -> String {
    match lit {
        Literal::Hex(n) => format!("0x{:X}", n),
        Literal::Int(n) => n.to_string(),
        Literal::Ident(segs) if constants.is_local_bare_ident(segs) => segs.join("::"),
        Literal::Ident(segs) => resolve_ident_to_u64(segs, constants)
            .map(|value| value.to_string())
            .unwrap_or_else(|| segs.join("::")),
        other => rust_literal_str(other),
    }
}

fn rust_literal_str(lit: &Literal) -> String {
    match lit {
        Literal::Hex(_) | Literal::Int(_) | Literal::Bool(_) | Literal::Float(_) => {
            rust_scalar_literal_str(lit)
        }
        Literal::Str(s) => format!("{:?}", s),
        Literal::Ident(segments) => segments.join("::"),
    }
}

fn rust_scalar_literal_str(lit: &Literal) -> String {
    if let Literal::Hex(n) = lit {
        return format!("0x{:X}", n);
    }
    if let Literal::Int(n) = lit {
        return n.to_string();
    }
    if let Literal::Bool(b) = lit {
        return b.to_string();
    }
    if let Literal::Float(f) = lit {
        return rust_float_literal_str(*f);
    }
    unreachable!("non-scalar literal passed to rust_scalar_literal_str")
}

fn rust_float_literal_str(value: f64) -> String {
    let s = format!("{}", value);
    if s.contains('.') || s.contains('e') {
        s
    } else {
        format!("{}.0", s)
    }
}

fn rust_typed_literal_str(lit: &Literal, ty: &TypeExpr) -> String {
    match (lit, &ty.base) {
        (Literal::Hex(n), BaseType::Primitive(p)) => rust_hex_str(*n, *p),
        _ => rust_literal_str(lit),
    }
}

fn rust_hex_str(value: u64, ty: PrimitiveType) -> String {
    match ty {
        PrimitiveType::U8 | PrimitiveType::I8 => format!("0x{:02X}", value),
        PrimitiveType::U16 | PrimitiveType::I16 => format!("0x{:04X}", value),
        PrimitiveType::U32 | PrimitiveType::I32 => format!("0x{:08X}", value),
        PrimitiveType::U64 | PrimitiveType::I64 => format!("0x{:016X}", value),
        PrimitiveType::F32 | PrimitiveType::F64 | PrimitiveType::Bool | PrimitiveType::Bytes => {
            format!("0x{:X}", value)
        }
    }
}
