use synapse_parser::ast::{
    ArraySuffix, BaseType, ConstDecl, EnumDef, FieldDef, Item, MessageDef, PrimitiveType,
    StructDef, SynFile, TypeExpr,
};

use crate::{
    constants::{ConstContext, const_context},
    error::CodegenError,
    types::{CfsOptions, PREAMBLE, ResolvedConstants},
    util::{
        emit_doc_lines, emit_indented_doc_lines, find_cc_attr, find_mid_attr, import_c_header,
        literal_cc_str, literal_mid_str, packet_is_command, packet_item, to_screaming_snake,
        typed_literal_str,
    },
    validate::validate_supported,
};

/// Generate a NASA cFS C header (`*_msg.h` + MID `#define`s) from a parsed Synapse file.
pub fn generate_c(file: &SynFile) -> String {
    try_generate_c(file).expect("parsed Synapse file is not supported by cFS C codegen")
}

/// Try to generate a NASA cFS C header (`*_msg.h` + MID `#define`s`) from a parsed Synapse file.
pub fn try_generate_c(file: &SynFile) -> Result<String, CodegenError> {
    try_generate_c_with_constants(file, &ResolvedConstants::new())
}

/// Try to generate a NASA cFS C header with validation options.
pub fn try_generate_c_with_options(
    file: &SynFile,
    options: &CfsOptions,
) -> Result<String, CodegenError> {
    try_generate_c_with_constants_and_options(file, &ResolvedConstants::new(), options)
}

/// Try to generate a C header with additional imported constants available for attributes.
pub fn try_generate_c_with_constants(
    file: &SynFile,
    imported_constants: &ResolvedConstants,
) -> Result<String, CodegenError> {
    try_generate_c_with_constants_and_options(file, imported_constants, &CfsOptions::default())
}

/// Try to generate a C header with imported constants and validation options.
pub fn try_generate_c_with_constants_and_options(
    file: &SynFile,
    imported_constants: &ResolvedConstants,
    options: &CfsOptions,
) -> Result<String, CodegenError> {
    let constants = const_context(file, imported_constants);
    validate_supported(file, &constants, options)?;
    let mut out = String::from(PREAMBLE);
    emit_c_imports(file, &mut out);
    emit_items(file, &mut out, &constants);
    Ok(out)
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

fn emit_items(file: &SynFile, out: &mut String, constants: &ConstContext<'_>) {
    emit_mid_defines(file, out, constants);
    emit_command_code_defines(file, out, constants);
    emit_enum_aliases(file, out);
    emit_c_types(file, out);
}

fn emit_mid_defines(file: &SynFile, out: &mut String, constants: &ConstContext<'_>) {
    let defines: Vec<_> = file
        .items
        .iter()
        .filter_map(|item| mid_define(item, constants))
        .collect();

    if defines.is_empty() {
        return;
    }

    out.push_str("/* Message IDs */\n");
    for define in defines {
        out.push_str(&define);
    }
    out.push('\n');
}

fn mid_define(item: &Item, constants: &ConstContext<'_>) -> Option<String> {
    let packet = packet_item(item)?;
    let mid = find_mid_attr(&packet.attrs)?;
    let define_name = to_screaming_snake(&packet.name);
    let mid_str = literal_mid_str(mid, constants);
    Some(format!("#define {}_MID  {}\n", define_name, mid_str))
}

fn emit_command_code_defines(file: &SynFile, out: &mut String, constants: &ConstContext<'_>) {
    let defines: Vec<_> = file
        .items
        .iter()
        .filter_map(|item| command_code_define(item, constants))
        .collect();

    if defines.is_empty() {
        return;
    }

    out.push_str("/* Command Codes */\n");
    for define in defines {
        out.push_str(&define);
    }
    out.push('\n');
}

fn command_code_define(item: &Item, constants: &ConstContext<'_>) -> Option<String> {
    let Item::Command(packet) = item else {
        return None;
    };
    let cc = find_cc_attr(&packet.attrs)?;
    let define_name = to_screaming_snake(&packet.name);
    let cc_str = literal_cc_str(cc, constants);
    Some(format!("#define {}_CC   {}\n", define_name, cc_str))
}

fn emit_enum_aliases(file: &SynFile, out: &mut String) {
    let mut namespace = Vec::new();
    for item in &file.items {
        match item {
            Item::Namespace(ns) => namespace = ns.name.clone(),
            Item::Enum(e) => emit_enum(out, e, &namespace),
            Item::Import(_)
            | Item::Const(_)
            | Item::Struct(_)
            | Item::Table(_)
            | Item::Command(_)
            | Item::Telemetry(_)
            | Item::Message(_) => {}
        }
    }
}

fn emit_c_types(file: &SynFile, out: &mut String) {
    let mut namespace = Vec::new();
    for item in &file.items {
        if update_namespace(&mut namespace, item) {
            continue;
        }
        emit_c_type(out, item, &namespace);
    }
}

fn update_namespace(namespace: &mut Vec<String>, item: &Item) -> bool {
    if let Item::Namespace(ns) = item {
        *namespace = ns.name.clone();
        true
    } else {
        false
    }
}

fn emit_c_type(out: &mut String, item: &Item, namespace: &[String]) {
    match item {
        Item::Const(c) => emit_const(out, c),
        Item::Struct(s) | Item::Table(s) => emit_struct(out, s, namespace),
        Item::Command(m) | Item::Telemetry(m) | Item::Message(m) => emit_message(out, m, namespace),
        Item::Namespace(_) | Item::Import(_) | Item::Enum(_) => {}
    }
}

fn emit_const(out: &mut String, c: &ConstDecl) {
    emit_doc_lines(out, &c.doc);
    let val = typed_literal_str(&c.value, &c.ty);
    out.push_str(&format!("#define {}  {}\n\n", c.name, val));
}

fn emit_enum(out: &mut String, e: &EnumDef, namespace: &[String]) {
    let Some(repr) = e.repr else {
        return;
    };

    let type_name = c_decl_type_name(&e.name, namespace);
    emit_doc_lines(out, &e.doc);
    out.push_str(&format!("typedef {} {};\n", primitive_str(repr), type_name));

    let enum_prefix = c_enum_variant_prefix(&e.name, namespace);
    for variant in &e.variants {
        emit_doc_lines(out, &variant.doc);
        let value = variant
            .value
            .expect("represented enum variants validated before emission");
        out.push_str(&format!(
            "#define {}_{}  (({}){})\n",
            enum_prefix,
            to_screaming_snake(&variant.name),
            type_name,
            value
        ));
    }
    out.push('\n');
}

fn emit_struct(out: &mut String, s: &StructDef, namespace: &[String]) {
    emit_doc_lines(out, &s.doc);
    out.push_str("typedef struct {\n");
    for f in &s.fields {
        emit_c_field(out, f, namespace);
    }
    out.push_str(&format!("}} {};\n\n", c_decl_type_name(&s.name, namespace)));
}

fn emit_message(out: &mut String, m: &MessageDef, namespace: &[String]) {
    let header_type = if packet_is_command(m) {
        "CFE_MSG_CommandHeader_t"
    } else {
        "CFE_MSG_TelemetryHeader_t"
    };

    emit_doc_lines(out, &m.doc);

    out.push_str("typedef struct {\n");
    out.push_str(&format!("    {} Header;\n", header_type));
    for f in &m.fields {
        emit_c_field(out, f, namespace);
    }
    out.push_str(&format!("}} {};\n\n", c_decl_type_name(&m.name, namespace)));
}

fn non_fixed_type_str(ty: &TypeExpr, namespace: &[String]) -> String {
    if ty.base == BaseType::String {
        return non_fixed_string_type_str(&ty.array);
    }

    let base = base_type_str(&ty.base, namespace);
    non_fixed_array_type_str(base, &ty.array)
}

fn non_fixed_string_type_str(array: &Option<ArraySuffix>) -> String {
    match array {
        None | Some(ArraySuffix::Dynamic) => "const char*".to_string(),
        Some(ArraySuffix::Fixed(_)) => unreachable!("handled by emit_c_field"),
        Some(ArraySuffix::Bounded(n)) => format!("char[{}]", n),
    }
}

fn non_fixed_array_type_str(base: String, array: &Option<ArraySuffix>) -> String {
    match array {
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

fn emit_c_field(out: &mut String, f: &FieldDef, namespace: &[String]) {
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

pub(crate) fn c_enum_variant_prefix(name: &str, namespace: &[String]) -> String {
    let mut segments = namespace.to_vec();
    segments.push(name.to_string());
    segments
        .iter()
        .map(|segment| to_screaming_snake(segment))
        .collect::<Vec<_>>()
        .join("_")
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

fn primitive_str(p: PrimitiveType) -> &'static str {
    const C_TYPES: &[(PrimitiveType, &str)] = &[
        (PrimitiveType::F32, "float"),
        (PrimitiveType::F64, "double"),
        (PrimitiveType::I8, "int8_t"),
        (PrimitiveType::I16, "int16_t"),
        (PrimitiveType::I32, "int32_t"),
        (PrimitiveType::I64, "int64_t"),
        (PrimitiveType::U8, "uint8_t"),
        (PrimitiveType::U16, "uint16_t"),
        (PrimitiveType::U32, "uint32_t"),
        (PrimitiveType::U64, "uint64_t"),
        (PrimitiveType::Bool, "bool"),
        (PrimitiveType::Bytes, "uint8_t*"),
    ];

    C_TYPES
        .iter()
        .find_map(|(ty, name)| (*ty == p).then_some(*name))
        .expect("all primitive types have C names")
}
