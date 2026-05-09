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
    /// cFS Software Bus command and telemetry packets require explicit message IDs.
    MissingMid { packet: String },
    /// Message IDs are only meaningful for cFS command and telemetry packets.
    MessageIdUnsupported { item: String },
    /// Message IDs must resolve to non-negative integers for cFS codegen.
    MessageIdValueUnsupported { packet: String },
    /// cFS command packets require an explicit command code.
    MissingCommandCode { packet: String },
    /// Command codes are only meaningful for cFS command packets.
    CommandCodeUnsupported { item: String },
    /// Command codes must be literal non-negative integers for cFS codegen today.
    CommandCodeValueUnsupported { packet: String },
    /// Literal MIDs must be unique within one generated file.
    DuplicateMid {
        mid: String,
        first_packet: String,
        second_packet: String,
    },
    /// Literal command MID/CC pairs must be unique within one generated file.
    DuplicateCommandCode {
        mid: String,
        cc: String,
        first_packet: String,
        second_packet: String,
    },
    /// Literal command/telemetry MIDs must match the expected cFS command bit pattern.
    MidRangeMismatch {
        packet: String,
        mid: String,
        expected: &'static str,
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
            CodegenError::UnboundedStringUnsupported { container, field } => write!(
                f,
                "unbounded string field `{container}.{field}` is not supported by cFS codegen; use `string[<=N]` or `string[N]`"
            ),
            CodegenError::LegacyMessageUnsupported { packet } => write!(
                f,
                "legacy message `{packet}` is not supported by cFS codegen; use `command` or `telemetry`"
            ),
            CodegenError::MissingMid { packet } => {
                write!(f, "packet `{packet}` is missing required `@mid(...)`")
            }
            CodegenError::MessageIdUnsupported { item } => write!(
                f,
                "`@mid(...)` is only supported on command and telemetry packets, found on `{item}`"
            ),
            CodegenError::MessageIdValueUnsupported { packet } => write!(
                f,
                "packet `{packet}` has unresolved or non-integer `@mid(...)`; cFS codegen requires an integer, hex, local integer constant, or imported integer constant message ID"
            ),
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
            CodegenError::DuplicateMid {
                mid,
                first_packet,
                second_packet,
            } => write!(
                f,
                "duplicate MID `{mid}` used by packets `{first_packet}` and `{second_packet}`"
            ),
            CodegenError::DuplicateCommandCode {
                mid,
                cc,
                first_packet,
                second_packet,
            } => write!(
                f,
                "duplicate command MID/CC pair `{mid}`/`{cc}` used by packets `{first_packet}` and `{second_packet}`"
            ),
            CodegenError::MidRangeMismatch {
                packet,
                mid,
                expected,
            } => write!(f, "packet `{packet}` has MID `{mid}`, expected {expected}"),
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
        }
    }
}

impl StdError for CodegenError {}
