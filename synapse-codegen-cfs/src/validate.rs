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
        enum_defs, file_namespace, find_cc_attr, find_mid_attr, literal_cc_str, primitive_name,
        type_expr_display,
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
/// This validates logical-topic and command-code attributes, but it does not
/// validate fields or other cFS ABI constraints.
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
    let mut command_group_codes = HashMap::new();
    for item in &file.items {
        validate_item(item, constants, &enum_defs, &mut command_group_codes)?;
    }
    Ok(())
}

fn validate_item(
    item: &Item,
    constants: &ConstContext<'_>,
    enum_defs: &HashMap<String, &EnumDef>,
    command_group_codes: &mut HashMap<(String, u64), String>,
) -> Result<(), CodegenError> {
    match item {
        Item::Struct(s) | Item::Table(s) => validate_plain_item(s, enum_defs),
        Item::Command(m) => validate_command_item(m, constants, enum_defs, command_group_codes),
        Item::Telemetry(m) => validate_telemetry_item(m, enum_defs),
        _ => validate_non_packet_item(item),
    }
}

fn validate_command_item(
    command: &MessageDef,
    constants: &ConstContext<'_>,
    enum_defs: &HashMap<String, &EnumDef>,
    command_group_codes: &mut HashMap<(String, u64), String>,
) -> Result<(), CodegenError> {
    reject_message_id(&command.name, &command.attrs)?;
    let group =
        command
            .command_group
            .as_ref()
            .ok_or_else(|| CodegenError::CommandGroupRequired {
                packet: command.name.clone(),
            })?;
    let cc = required_command_code(command)?;
    let cc_value = resolved_command_code(command, cc, constants)?;
    let key = (group.clone(), cc_value);
    if let Some(first_packet) = command_group_codes.insert(key, command.name.clone()) {
        return Err(CodegenError::DuplicateCommandCodeInGroup {
            group: group.clone(),
            cc: literal_cc_str(cc, constants),
            first_packet,
            second_packet: command.name.clone(),
        });
    }

    validate_fields(&command.name, &command.fields, enum_defs)
}

fn validate_telemetry_item(
    telemetry: &MessageDef,
    enum_defs: &HashMap<String, &EnumDef>,
) -> Result<(), CodegenError> {
    reject_message_id(&telemetry.name, &telemetry.attrs)?;
    reject_telemetry_command_code(telemetry)?;
    validate_fields(&telemetry.name, &telemetry.fields, enum_defs)
}

fn validate_non_packet_item(item: &Item) -> Result<(), CodegenError> {
    match item {
        Item::Message(m) => Err(CodegenError::LegacyMessageUnsupported {
            packet: m.name.clone(),
        }),
        Item::Enum(e) => validate_enum(e),
        Item::Namespace(_) | Item::Import(_) | Item::Const(_) => Ok(()),
        Item::Struct(_) | Item::Table(_) | Item::Command(_) | Item::Telemetry(_) => {
            unreachable!("packet and plain items handled before validate_non_packet_item")
        }
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

    reject_message_id(&packet.name, &packet.attrs)?;
    let (kind, cc_value) = collected_packet_kind(packet, constants)?;
    let topic = match packet.kind {
        PacketKind::Command => {
            packet
                .command_group
                .clone()
                .ok_or_else(|| CodegenError::CommandGroupRequired {
                    packet: packet.name.clone(),
                })?
        }
        PacketKind::Telemetry => packet.name.clone(),
        PacketKind::Message => unreachable!("legacy message items are not collected"),
    };

    Ok(Some(CfsPacket {
        namespace: namespace.to_vec(),
        name: packet.name.clone(),
        kind,
        topic,
        cc: cc_value,
    }))
}

fn collected_packet_kind(
    packet: &MessageDef,
    constants: &ConstContext<'_>,
) -> Result<(CfsPacketKind, Option<u64>), CodegenError> {
    if packet.kind == PacketKind::Command {
        return collected_command_packet_kind(packet, constants);
    }
    if packet.kind == PacketKind::Telemetry {
        return collected_telemetry_packet_kind(packet);
    }
    unreachable!("legacy message items are not collected")
}

fn collected_command_packet_kind(
    packet: &MessageDef,
    constants: &ConstContext<'_>,
) -> Result<(CfsPacketKind, Option<u64>), CodegenError> {
    Ok((
        CfsPacketKind::Command,
        Some(required_command_code_value(packet, constants)?),
    ))
}

fn collected_telemetry_packet_kind(
    packet: &MessageDef,
) -> Result<(CfsPacketKind, Option<u64>), CodegenError> {
    reject_telemetry_command_code(packet)?;
    Ok((CfsPacketKind::Telemetry, None))
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

fn validate_plain_item_attrs(item_name: &str, attrs: &[Attribute]) -> Result<(), CodegenError> {
    reject_message_id(item_name, attrs)?;
    if find_cc_attr(attrs).is_some() {
        return Err(CodegenError::CommandCodeUnsupported {
            item: item_name.to_string(),
        });
    }
    Ok(())
}

fn reject_message_id(item_name: &str, attrs: &[Attribute]) -> Result<(), CodegenError> {
    if find_mid_attr(attrs).is_some() {
        return Err(CodegenError::MessageIdUnsupported {
            item: item_name.to_string(),
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
