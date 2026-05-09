use std::collections::HashMap;

use synapse_parser::ast::{
    ArraySuffix, Attribute, BaseType, EnumDef, FieldDef, Item, Literal, MessageDef, PacketKind,
    PrimitiveType, SynFile,
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
        match item {
            Item::Struct(s) | Item::Table(s) => {
                validate_plain_item_attrs(&s.name, &s.attrs)?;
                validate_fields(&s.name, &s.fields, &enum_defs)?
            }
            Item::Command(m) | Item::Telemetry(m) => {
                validate_packet(m, constants, &mut telemetry_mids, &mut command_codes)?;
                validate_fields(&m.name, &m.fields, &enum_defs)?
            }
            Item::Message(m) => {
                return Err(CodegenError::LegacyMessageUnsupported {
                    packet: m.name.clone(),
                });
            }
            Item::Enum(e) => validate_enum(e)?,
            Item::Namespace(_) | Item::Import(_) | Item::Const(_) => {}
        }
    }
    Ok(())
}

fn collect_cfs_packets(
    file: &SynFile,
    constants: &ConstContext<'_>,
) -> Result<Vec<CfsPacket>, CodegenError> {
    let namespace = file_namespace(file);
    let mut packets = Vec::new();

    for item in &file.items {
        let packet = match item {
            Item::Command(m) | Item::Telemetry(m) => m,
            Item::Namespace(_)
            | Item::Import(_)
            | Item::Const(_)
            | Item::Enum(_)
            | Item::Struct(_)
            | Item::Table(_)
            | Item::Message(_) => continue,
        };

        let Some(mid) = find_mid_attr(&packet.attrs) else {
            return Err(CodegenError::MissingMid {
                packet: packet.name.clone(),
            });
        };
        let mid_value = resolve_literal_to_u64(mid, constants).ok_or_else(|| {
            CodegenError::MessageIdValueUnsupported {
                packet: packet.name.clone(),
            }
        })?;
        validate_mid_range(packet, mid_value, mid, constants)?;

        let cc = find_cc_attr(&packet.attrs);
        let (kind, cc_value) = match packet.kind {
            PacketKind::Command => {
                let cc = cc.ok_or_else(|| CodegenError::MissingCommandCode {
                    packet: packet.name.clone(),
                })?;
                let cc_value = resolve_literal_to_u64(cc, constants).ok_or_else(|| {
                    CodegenError::CommandCodeValueUnsupported {
                        packet: packet.name.clone(),
                    }
                })?;
                (CfsPacketKind::Command, Some(cc_value))
            }
            PacketKind::Telemetry => {
                if cc.is_some() {
                    return Err(CodegenError::CommandCodeUnsupported {
                        item: packet.name.clone(),
                    });
                }
                (CfsPacketKind::Telemetry, None)
            }
            PacketKind::Message => continue,
        };

        packets.push(CfsPacket {
            namespace: namespace.clone(),
            name: packet.name.clone(),
            kind,
            mid: mid_value,
            cc: cc_value,
        });
    }

    Ok(packets)
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
    match repr {
        PrimitiveType::I8 => Some((i8::MIN as i64, i8::MAX as i64)),
        PrimitiveType::I16 => Some((i16::MIN as i64, i16::MAX as i64)),
        PrimitiveType::I32 => Some((i32::MIN as i64, i32::MAX as i64)),
        PrimitiveType::I64 => Some((i64::MIN, i64::MAX)),
        PrimitiveType::U8 => Some((0, u8::MAX as i64)),
        PrimitiveType::U16 => Some((0, u16::MAX as i64)),
        PrimitiveType::U32 => Some((0, u32::MAX as i64)),
        PrimitiveType::U64 => Some((0, i64::MAX)),
        PrimitiveType::F32 | PrimitiveType::F64 | PrimitiveType::Bool | PrimitiveType::Bytes => {
            None
        }
    }
}

fn validate_packet(
    packet: &MessageDef,
    constants: &ConstContext<'_>,
    telemetry_mids: &mut HashMap<u64, String>,
    command_codes: &mut HashMap<(u64, u64), String>,
) -> Result<(), CodegenError> {
    let Some(mid) = find_mid_attr(&packet.attrs) else {
        return Err(CodegenError::MissingMid {
            packet: packet.name.clone(),
        });
    };

    let cc = find_cc_attr(&packet.attrs);
    match packet.kind {
        PacketKind::Command if cc.is_none() => {
            return Err(CodegenError::MissingCommandCode {
                packet: packet.name.clone(),
            });
        }
        PacketKind::Telemetry if cc.is_some() => {
            return Err(CodegenError::CommandCodeUnsupported {
                item: packet.name.clone(),
            });
        }
        PacketKind::Command | PacketKind::Telemetry | PacketKind::Message => {}
    }
    let cc_value = if packet.kind == PacketKind::Command {
        let cc = cc.expect("command code was checked above");
        Some(resolve_literal_to_u64(cc, constants).ok_or_else(|| {
            CodegenError::CommandCodeValueUnsupported {
                packet: packet.name.clone(),
            }
        })?)
    } else {
        None
    };

    let value = resolve_literal_to_u64(mid, constants).ok_or_else(|| {
        CodegenError::MessageIdValueUnsupported {
            packet: packet.name.clone(),
        }
    })?;

    validate_mid_range(packet, value, mid, constants)?;
    match packet.kind {
        PacketKind::Command => {
            let cc = cc.expect("command code was checked above");
            let cc_value = cc_value.expect("command code value was checked above");
            if let Some(first_packet) = command_codes.insert((value, cc_value), packet.name.clone())
            {
                return Err(CodegenError::DuplicateCommandCode {
                    mid: literal_mid_str(mid, constants),
                    cc: literal_cc_str(cc, constants),
                    first_packet,
                    second_packet: packet.name.clone(),
                });
            }
        }
        PacketKind::Telemetry => {
            if let Some(first_packet) = telemetry_mids.insert(value, packet.name.clone()) {
                return Err(CodegenError::DuplicateMid {
                    mid: literal_mid_str(mid, constants),
                    first_packet,
                    second_packet: packet.name.clone(),
                });
            }
        }
        PacketKind::Message => {}
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
        if field.ty.base == BaseType::String && field.ty.array.is_none() {
            return Err(CodegenError::UnboundedStringUnsupported {
                container: container.to_string(),
                field: field.name.clone(),
            });
        }
        if let BaseType::Ref(segments) = &field.ty.base {
            if let Some(e) = segments
                .last()
                .and_then(|name| enum_defs.get(name.as_str()))
            {
                if e.repr.is_none() {
                    return Err(CodegenError::EnumFieldUnsupported {
                        container: container.to_string(),
                        field: field.name.clone(),
                        ty: segments.join("::"),
                    });
                }
            }
        }
        match &field.ty.array {
            Some(ArraySuffix::Dynamic) => {
                return Err(CodegenError::DynamicArrayUnsupported {
                    container: container.to_string(),
                    field: field.name.clone(),
                    ty: type_expr_display(&field.ty),
                });
            }
            Some(ArraySuffix::Bounded(_)) if field.ty.base != BaseType::String => {
                return Err(CodegenError::BoundedArrayUnsupported {
                    container: container.to_string(),
                    field: field.name.clone(),
                    ty: type_expr_display(&field.ty),
                });
            }
            Some(ArraySuffix::Bounded(_)) | Some(ArraySuffix::Fixed(_)) | None => {}
        }
    }
    Ok(())
}
