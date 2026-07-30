use std::{error::Error as StdError, fmt};

/// Error returned when a parsed Synapse file cannot be emitted safely.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodegenError {
    /// Optional fields parse today, but cFS ABI codegen has no representation for them yet.
    OptionalFieldUnsupported { container: String, field: String },
    /// Field defaults parse today, but cFS ABI codegen does not generate initializers yet.
    DefaultValueUnsupported { container: String, field: String },
    /// Unrepresented enum fields parse today, but cFS ABI codegen needs an explicit representation.
    EnumFieldUnsupported {
        container: String,
        field: String,
        ty: String,
    },
    /// Represented enums must use integer ABI types.
    EnumRepresentationUnsupported { enum_name: String, repr: String },
    /// Represented enums require explicit values for every variant.
    EnumVariantValueRequired { enum_name: String, variant: String },
    /// Represented enum variant values must fit the selected ABI type.
    EnumVariantValueOutOfRange {
        enum_name: String,
        variant: String,
        value: i64,
        repr: String,
    },
    /// Unbounded strings would generate pointer fields, which are not cFS packet/table ABI data.
    UnboundedStringUnsupported { container: String, field: String },
    /// The legacy `message` keyword is parsed for migration, but cFS codegen requires intent.
    LegacyMessageUnsupported { packet: String },
    /// Deployment message IDs do not belong in reusable schemas.
    MessageIdUnsupported { item: String },
    /// Commands must belong to a logical command topic.
    CommandGroupRequired { packet: String },
    /// cFS command packets require an explicit command code.
    MissingCommandCode { packet: String },
    /// Command codes are only meaningful for cFS command packets.
    CommandCodeUnsupported { item: String },
    /// Command codes must be literal non-negative integers for cFS codegen today.
    CommandCodeValueUnsupported { packet: String },
    /// Generated command-code constants use the cFE function-code ABI type.
    CommandCodeOutOfRange { packet: String, value: u64 },
    /// Function codes must be unique within one logical command topic.
    DuplicateCommandCodeInGroup {
        group: String,
        cc: String,
        first_packet: String,
        second_packet: String,
    },
    /// Dynamic arrays parse today, but cFS ABI codegen has no ownership/length model yet.
    DynamicArrayUnsupported {
        container: String,
        field: String,
        ty: String,
    },
    /// Non-string bounded arrays parse today, but cFS ABI codegen has no inline representation yet.
    BoundedArrayUnsupported {
        container: String,
        field: String,
        ty: String,
    },
}

impl fmt::Display for CodegenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CodegenError::OptionalFieldUnsupported { .. }
            | CodegenError::DefaultValueUnsupported { .. }
            | CodegenError::UnboundedStringUnsupported { .. }
            | CodegenError::DynamicArrayUnsupported { .. }
            | CodegenError::BoundedArrayUnsupported { .. } => fmt_field_error(self, f),
            CodegenError::EnumFieldUnsupported { .. }
            | CodegenError::EnumRepresentationUnsupported { .. }
            | CodegenError::EnumVariantValueRequired { .. }
            | CodegenError::EnumVariantValueOutOfRange { .. } => fmt_enum_error(self, f),
            CodegenError::LegacyMessageUnsupported { .. }
            | CodegenError::MessageIdUnsupported { .. }
            | CodegenError::CommandGroupRequired { .. } => fmt_packet_error(self, f),
            CodegenError::MissingCommandCode { .. }
            | CodegenError::CommandCodeUnsupported { .. }
            | CodegenError::CommandCodeValueUnsupported { .. }
            | CodegenError::CommandCodeOutOfRange { .. } => fmt_command_error(self, f),
            CodegenError::DuplicateCommandCodeInGroup { .. } => fmt_duplicate_error(self, f),
        }
    }
}

fn fmt_field_error(error: &CodegenError, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match error {
        CodegenError::OptionalFieldUnsupported { .. }
        | CodegenError::DefaultValueUnsupported { .. }
        | CodegenError::UnboundedStringUnsupported { .. } => fmt_scalar_field_error(error, f),
        CodegenError::DynamicArrayUnsupported { .. }
        | CodegenError::BoundedArrayUnsupported { .. } => fmt_array_field_error(error, f),
        _ => unreachable!("non-field error passed to fmt_field_error"),
    }
}

fn fmt_scalar_field_error(error: &CodegenError, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match error {
        CodegenError::OptionalFieldUnsupported { container, field } => write!(
            f,
            "optional field `{container}.{field}` is not supported by cFS codegen yet"
        ),
        CodegenError::DefaultValueUnsupported { container, field } => write!(
            f,
            "default value for field `{container}.{field}` is not supported by cFS codegen yet"
        ),
        CodegenError::UnboundedStringUnsupported { container, field } => write!(
            f,
            "unbounded string field `{container}.{field}` is not supported by cFS codegen; use `string[<=N]` or `string[N]`"
        ),
        _ => unreachable!("non-scalar field error passed to fmt_scalar_field_error"),
    }
}

fn fmt_array_field_error(error: &CodegenError, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match error {
        CodegenError::DynamicArrayUnsupported {
            container,
            field,
            ty,
        } => write!(
            f,
            "dynamic array field `{container}.{field}` with type `{ty}` is not supported by cFS codegen yet"
        ),
        CodegenError::BoundedArrayUnsupported {
            container,
            field,
            ty,
        } => write!(
            f,
            "bounded array field `{container}.{field}` with type `{ty}` is not supported by cFS codegen yet"
        ),
        _ => unreachable!("non-array field error passed to fmt_array_field_error"),
    }
}

fn fmt_enum_error(error: &CodegenError, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match error {
        CodegenError::EnumFieldUnsupported {
            container,
            field,
            ty,
        } => write!(
            f,
            "enum field `{container}.{field}` with type `{ty}` needs an explicit integer representation for cFS codegen"
        ),
        CodegenError::EnumRepresentationUnsupported { enum_name, repr } => write!(
            f,
            "enum `{enum_name}` uses unsupported representation `{repr}`; cFS codegen supports integer enum representations"
        ),
        CodegenError::EnumVariantValueRequired { enum_name, variant } => write!(
            f,
            "enum `{enum_name}` variant `{variant}` needs an explicit value for cFS codegen"
        ),
        CodegenError::EnumVariantValueOutOfRange {
            enum_name,
            variant,
            value,
            repr,
        } => write!(
            f,
            "enum `{enum_name}` variant `{variant}` value `{value}` does not fit `{repr}`"
        ),
        _ => unreachable!("non-enum error passed to fmt_enum_error"),
    }
}

fn fmt_packet_error(error: &CodegenError, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match error {
        CodegenError::LegacyMessageUnsupported { packet } => write!(
            f,
            "legacy message `{packet}` is not supported by cFS codegen; use `command` or `telemetry`"
        ),
        CodegenError::MessageIdUnsupported { item } => write!(
            f,
            "`@mid(...)` is not supported on `{item}`; assign its logical topic in the mission manifest"
        ),
        CodegenError::CommandGroupRequired { packet } => write!(
            f,
            "command `{packet}` must be declared inside a `commands` group"
        ),
        _ => unreachable!("non-packet error passed to fmt_packet_error"),
    }
}

fn fmt_command_error(error: &CodegenError, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match error {
        CodegenError::MissingCommandCode { packet } => {
            write!(f, "command `{packet}` is missing required `@cc(...)`")
        }
        CodegenError::CommandCodeUnsupported { item } => write!(
            f,
            "`@cc(...)` is only supported on command packets, found on `{item}`"
        ),
        CodegenError::CommandCodeValueUnsupported { packet } => write!(
            f,
            "command `{packet}` has unresolved or non-integer `@cc(...)`; cFS codegen requires an integer, hex, or local integer constant command code"
        ),
        CodegenError::CommandCodeOutOfRange { packet, value } => write!(
            f,
            "command `{packet}` has function code `{value}` outside the supported `u16` range"
        ),
        _ => unreachable!("non-command error passed to fmt_command_error"),
    }
}

fn fmt_duplicate_error(error: &CodegenError, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match error {
        CodegenError::DuplicateCommandCodeInGroup {
            group,
            cc,
            first_packet,
            second_packet,
        } => write!(
            f,
            "duplicate function code `{cc}` in command topic `{group}` used by commands `{first_packet}` and `{second_packet}`"
        ),
        _ => unreachable!("non-duplicate error passed to fmt_duplicate_error"),
    }
}

impl StdError for CodegenError {}
