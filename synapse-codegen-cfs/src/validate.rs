use std::collections::HashMap;

use synapse_parser::ast::{
    ArraySuffix, Attribute, BaseType, EnumDef, FieldDef, Item, Literal, MessageDef, PacketKind,
    PrimitiveType, StructDef, SynFile,
};

use crate::{
    constants::{ConstContext, const_context, resolve_literal_to_u64},
    error::CodegenError,
    types::{CfsPacket, CfsPacketKind, ResolvedConstants},
    util::{
        enum_defs, file_namespace, find_cc_attr, find_mid_attr, literal_cc_str, literal_mid_str,
        primitive_name, type_expr_display,
    },
};

/// Validate that a parsed Synapse file is supported by cFS code generation.
pub fn validate_cfs(file: &SynFile) -> Result<(), CodegenError> {
    validate_cfs_with_constants(file, &ResolvedConstants::new())
}

/// Validate cFS code generation support with additional imported constants available.
pub fn validate_cfs_with_constants(
    file: &SynFile,
    imported_constants: &ResolvedConstants,
) -> Result<(), CodegenError> {
    let constants = const_context(file, imported_constants);
    validate_supported(file, &constants)
}

/// Collect resolved cFS packet facts with additional imported constants available.
///
/// This validates packet-level attributes needed to resolve MIDs and command
/// codes, but it does not validate fields or other cFS ABI constraints.
pub fn collect_cfs_packets_with_constants(
    file: &SynFile,
    imported_constants: &ResolvedConstants,
) -> Result<Vec<CfsPacket>, CodegenError> {
    let constants = const_context(file, imported_constants);
    collect_cfs_packets(file, &constants)
}

pub(crate) fn validate_supported(
    file: &SynFile,
    constants: &ConstContext<'_>,
) -> Result<(), CodegenError> {
    let enum_defs = enum_defs(file);
    let mut telemetry_mids = HashMap::new();
    let mut command_codes = HashMap::new();
    for item in &file.items {
        validate_item(
            item,
            constants,
            &enum_defs,
            &mut telemetry_mids,
            &mut command_codes,
        )?;
    }
    Ok(())
}

fn validate_item(
    item: &Item,
    constants: &ConstContext<'_>,
    enum_defs: &HashMap<String, &EnumDef>,
    telemetry_mids: &mut HashMap<u64, String>,
    command_codes: &mut HashMap<(u64, u64), String>,
) -> Result<(), CodegenError> {
    match item {
        Item::Struct(s) | Item::Table(s) => validate_plain_item(s, enum_defs),
        Item::Command(m) | Item::Telemetry(m) => {
            validate_packet(m, constants, telemetry_mids, command_codes)?;
            validate_fields(&m.name, &m.fields, enum_defs)
        }
        Item::Message(m) => Err(CodegenError::LegacyMessageUnsupported {
            packet: m.name.clone(),
        }),
        Item::Enum(e) => validate_enum(e),
        Item::Namespace(_) | Item::Import(_) | Item::Const(_) => Ok(()),
    }
}

fn validate_plain_item(
    item: &StructDef,
    enum_defs: &HashMap<String, &EnumDef>,
) -> Result<(), CodegenError> {
    validate_plain_item_attrs(&item.name, &item.attrs)?;
    validate_fields(&item.name, &item.fields, enum_defs)
}

fn collect_cfs_packets(
    file: &SynFile,
    constants: &ConstContext<'_>,
) -> Result<Vec<CfsPacket>, CodegenError> {
    let namespace = file_namespace(file);
    let mut packets = Vec::new();

    for item in &file.items {
        if let Some(packet) = cfs_packet_from_item(item, constants, &namespace)? {
            packets.push(packet);
        }
    }

    Ok(packets)
}

fn cfs_packet_from_item(
    item: &Item,
    constants: &ConstContext<'_>,
    namespace: &[String],
) -> Result<Option<CfsPacket>, CodegenError> {
    let (Item::Command(packet) | Item::Telemetry(packet)) = item else {
        return Ok(None);
    };

    let mid = required_mid(packet)?;
    let mid_value = resolved_mid(packet, mid, constants)?;
    validate_mid_range(packet, mid_value, mid, constants)?;
    let (kind, cc_value) = collected_packet_kind(packet, constants)?;

    Ok(Some(CfsPacket {
        namespace: namespace.to_vec(),
        name: packet.name.clone(),
        kind,
        mid: mid_value,
        cc: cc_value,
    }))
}

fn collected_packet_kind(
    packet: &MessageDef,
    constants: &ConstContext<'_>,
) -> Result<(CfsPacketKind, Option<u64>), CodegenError> {
    match packet.kind {
        PacketKind::Command => Ok((
            CfsPacketKind::Command,
            Some(required_command_code_value(packet, constants)?),
        )),
        PacketKind::Telemetry => {
            reject_telemetry_command_code(packet)?;
            Ok((CfsPacketKind::Telemetry, None))
        }
        PacketKind::Message => unreachable!("legacy message items are not collected"),
    }
}

fn validate_enum(e: &EnumDef) -> Result<(), CodegenError> {
    let Some(repr) = e.repr else {
        return Ok(());
    };
    let Some((min, max)) = enum_repr_range(repr) else {
        return Err(CodegenError::EnumRepresentationUnsupported {
            enum_name: e.name.clone(),
            repr: primitive_name(repr).to_string(),
        });
    };

    for variant in &e.variants {
        let value = variant
            .value
            .ok_or_else(|| CodegenError::EnumVariantValueRequired {
                enum_name: e.name.clone(),
                variant: variant.name.clone(),
            })?;
        if value < min || value > max {
            return Err(CodegenError::EnumVariantValueOutOfRange {
                enum_name: e.name.clone(),
                variant: variant.name.clone(),
                value,
                repr: primitive_name(repr).to_string(),
            });
        }
    }
    Ok(())
}

fn enum_repr_range(repr: PrimitiveType) -> Option<(i64, i64)> {
    const RANGES: &[(PrimitiveType, (i64, i64))] = &[
        (PrimitiveType::I8, (i8::MIN as i64, i8::MAX as i64)),
        (PrimitiveType::I16, (i16::MIN as i64, i16::MAX as i64)),
        (PrimitiveType::I32, (i32::MIN as i64, i32::MAX as i64)),
        (PrimitiveType::I64, (i64::MIN, i64::MAX)),
        (PrimitiveType::U8, (0, u8::MAX as i64)),
        (PrimitiveType::U16, (0, u16::MAX as i64)),
        (PrimitiveType::U32, (0, u32::MAX as i64)),
        (PrimitiveType::U64, (0, i64::MAX)),
    ];

    RANGES
        .iter()
        .find_map(|(ty, range)| (*ty == repr).then_some(*range))
}

fn required_mid(packet: &MessageDef) -> Result<&Literal, CodegenError> {
    find_mid_attr(&packet.attrs).ok_or_else(|| CodegenError::MissingMid {
        packet: packet.name.clone(),
    })
}

fn resolved_mid(
    packet: &MessageDef,
    mid: &Literal,
    constants: &ConstContext<'_>,
) -> Result<u64, CodegenError> {
    resolve_literal_to_u64(mid, constants).ok_or_else(|| CodegenError::MessageIdValueUnsupported {
        packet: packet.name.clone(),
    })
}

fn required_command_code(packet: &MessageDef) -> Result<&Literal, CodegenError> {
    find_cc_attr(&packet.attrs).ok_or_else(|| CodegenError::MissingCommandCode {
        packet: packet.name.clone(),
    })
}

fn resolved_command_code(
    packet: &MessageDef,
    cc: &Literal,
    constants: &ConstContext<'_>,
) -> Result<u64, CodegenError> {
    resolve_literal_to_u64(cc, constants).ok_or_else(|| CodegenError::CommandCodeValueUnsupported {
        packet: packet.name.clone(),
    })
}

fn required_command_code_value(
    packet: &MessageDef,
    constants: &ConstContext<'_>,
) -> Result<u64, CodegenError> {
    let cc = required_command_code(packet)?;
    resolved_command_code(packet, cc, constants)
}

fn reject_telemetry_command_code(packet: &MessageDef) -> Result<(), CodegenError> {
    if find_cc_attr(&packet.attrs).is_some() {
        return Err(CodegenError::CommandCodeUnsupported {
            item: packet.name.clone(),
        });
    }
    Ok(())
}

fn validate_packet(
    packet: &MessageDef,
    constants: &ConstContext<'_>,
    telemetry_mids: &mut HashMap<u64, String>,
    command_codes: &mut HashMap<(u64, u64), String>,
) -> Result<(), CodegenError> {
    let mid = required_mid(packet)?;
    validate_packet_command_code_shape(packet)?;
    let cc_value = optional_command_code_value(packet, constants)?;
    let value = resolved_mid(packet, mid, constants)?;
    validate_mid_range(packet, value, mid, constants)?;
    register_packet_mid(
        packet,
        constants,
        telemetry_mids,
        command_codes,
        value,
        cc_value,
    )
}

fn validate_packet_command_code_shape(packet: &MessageDef) -> Result<(), CodegenError> {
    match packet.kind {
        PacketKind::Command => required_command_code(packet).map(|_| ()),
        PacketKind::Telemetry => reject_telemetry_command_code(packet),
        PacketKind::Message => Ok(()),
    }
}

fn optional_command_code_value(
    packet: &MessageDef,
    constants: &ConstContext<'_>,
) -> Result<Option<u64>, CodegenError> {
    if packet.kind == PacketKind::Command {
        return Ok(Some(required_command_code_value(packet, constants)?));
    }
    Ok(None)
}

fn register_packet_mid(
    packet: &MessageDef,
    constants: &ConstContext<'_>,
    telemetry_mids: &mut HashMap<u64, String>,
    command_codes: &mut HashMap<(u64, u64), String>,
    value: u64,
    cc_value: Option<u64>,
) -> Result<(), CodegenError> {
    match packet.kind {
        PacketKind::Command => {
            register_command_mid(packet, constants, command_codes, value, cc_value)?;
        }
        PacketKind::Telemetry => register_telemetry_mid(packet, constants, telemetry_mids, value)?,
        PacketKind::Message => {}
    }

    Ok(())
}

fn register_command_mid(
    packet: &MessageDef,
    constants: &ConstContext<'_>,
    command_codes: &mut HashMap<(u64, u64), String>,
    value: u64,
    cc_value: Option<u64>,
) -> Result<(), CodegenError> {
    let cc = required_command_code(packet).expect("command code was checked above");
    let cc_value = cc_value.expect("command code value was checked above");
    if let Some(first_packet) = command_codes.insert((value, cc_value), packet.name.clone()) {
        return Err(CodegenError::DuplicateCommandCode {
            mid: literal_mid_str(
                required_mid(packet).expect("MID was checked above"),
                constants,
            ),
            cc: literal_cc_str(cc, constants),
            first_packet,
            second_packet: packet.name.clone(),
        });
    }
    Ok(())
}

fn register_telemetry_mid(
    packet: &MessageDef,
    constants: &ConstContext<'_>,
    telemetry_mids: &mut HashMap<u64, String>,
    value: u64,
) -> Result<(), CodegenError> {
    if let Some(first_packet) = telemetry_mids.insert(value, packet.name.clone()) {
        return Err(CodegenError::DuplicateMid {
            mid: literal_mid_str(
                required_mid(packet).expect("MID was checked above"),
                constants,
            ),
            first_packet,
            second_packet: packet.name.clone(),
        });
    }
    Ok(())
}

fn validate_plain_item_attrs(item_name: &str, attrs: &[Attribute]) -> Result<(), CodegenError> {
    if find_mid_attr(attrs).is_some() {
        return Err(CodegenError::MessageIdUnsupported {
            item: item_name.to_string(),
        });
    }
    if find_cc_attr(attrs).is_some() {
        return Err(CodegenError::CommandCodeUnsupported {
            item: item_name.to_string(),
        });
    }
    Ok(())
}

fn validate_mid_range(
    packet: &MessageDef,
    value: u64,
    mid: &Literal,
    constants: &ConstContext<'_>,
) -> Result<(), CodegenError> {
    let command_bit_set = (value & 0x1000) != 0;
    let expected = match packet.kind {
        PacketKind::Command if !command_bit_set => Some("command MID with bit 0x1000 set"),
        PacketKind::Telemetry if command_bit_set => Some("telemetry MID with bit 0x1000 clear"),
        PacketKind::Command | PacketKind::Telemetry | PacketKind::Message => None,
    };

    if let Some(expected) = expected {
        return Err(CodegenError::MidRangeMismatch {
            packet: packet.name.clone(),
            mid: literal_mid_str(mid, constants),
            expected,
        });
    }

    Ok(())
}

fn validate_fields(
    container: &str,
    fields: &[FieldDef],
    enum_defs: &HashMap<String, &EnumDef>,
) -> Result<(), CodegenError> {
    for field in fields {
        validate_field(container, field, enum_defs)?;
    }
    Ok(())
}

fn validate_field(
    container: &str,
    field: &FieldDef,
    enum_defs: &HashMap<String, &EnumDef>,
) -> Result<(), CodegenError> {
    validate_field_modifiers(container, field)?;
    validate_field_base(container, field, enum_defs)?;
    validate_field_array(container, field)
}

fn validate_field_modifiers(container: &str, field: &FieldDef) -> Result<(), CodegenError> {
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
    Ok(())
}

fn validate_field_base(
    container: &str,
    field: &FieldDef,
    enum_defs: &HashMap<String, &EnumDef>,
) -> Result<(), CodegenError> {
    validate_string_field(container, field)?;
    validate_enum_field(container, field, enum_defs)
}

fn validate_string_field(container: &str, field: &FieldDef) -> Result<(), CodegenError> {
    if field.ty.base == BaseType::String && field.ty.array.is_none() {
        return Err(CodegenError::UnboundedStringUnsupported {
            container: container.to_string(),
            field: field.name.clone(),
        });
    }
    Ok(())
}

fn validate_enum_field(
    container: &str,
    field: &FieldDef,
    enum_defs: &HashMap<String, &EnumDef>,
) -> Result<(), CodegenError> {
    let BaseType::Ref(segments) = &field.ty.base else {
        return Ok(());
    };
    let Some(e) = segments
        .last()
        .and_then(|name| enum_defs.get(name.as_str()))
    else {
        return Ok(());
    };
    if e.repr.is_none() {
        return Err(CodegenError::EnumFieldUnsupported {
            container: container.to_string(),
            field: field.name.clone(),
            ty: segments.join("::"),
        });
    }
    Ok(())
}

fn validate_field_array(container: &str, field: &FieldDef) -> Result<(), CodegenError> {
    match &field.ty.array {
        Some(ArraySuffix::Dynamic) => Err(CodegenError::DynamicArrayUnsupported {
            container: container.to_string(),
            field: field.name.clone(),
            ty: type_expr_display(&field.ty),
        }),
        Some(ArraySuffix::Bounded(_)) if field.ty.base != BaseType::String => {
            Err(CodegenError::BoundedArrayUnsupported {
                container: container.to_string(),
                field: field.name.clone(),
                ty: type_expr_display(&field.ty),
            })
        }
        Some(ArraySuffix::Bounded(_)) | Some(ArraySuffix::Fixed(_)) | None => Ok(()),
    }
}
