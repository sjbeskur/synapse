// ── Public types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct SynFile {
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Namespace(NamespaceDecl),
    Import(ImportDecl),
    Const(ConstDecl),
    Enum(EnumDef),
    Struct(StructDef),
    Table(StructDef),
    Command(MessageDef),
    Telemetry(MessageDef),
    Message(MessageDef),
}

#[derive(Debug, Clone, PartialEq)]
pub struct NamespaceDecl {
    pub name: ScopedIdent,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportDecl {
    pub path: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConstDecl {
    pub name: String,
    pub ty: TypeExpr,
    pub value: Literal,
    pub doc: Vec<String>,
    pub attrs: Vec<Attribute>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumDef {
    pub name: String,
    pub repr: Option<PrimitiveType>,
    pub variants: Vec<EnumVariant>,
    pub doc: Vec<String>,
    pub attrs: Vec<Attribute>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub name: String,
    pub value: Option<i64>,
    pub doc: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<FieldDef>,
    pub doc: Vec<String>,
    pub attrs: Vec<Attribute>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MessageDef {
    pub kind: PacketKind,
    /// Logical command topic containing this command.
    ///
    /// Telemetry and legacy messages do not belong to command groups.
    pub command_group: Option<String>,
    pub name: String,
    pub fields: Vec<FieldDef>,
    pub doc: Vec<String>,
    pub attrs: Vec<Attribute>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketKind {
    /// Legacy generic Software Bus packet. cFS codegen rejects this in favor of explicit packet kinds.
    Message,
    /// cFS Software Bus command packet.
    Command,
    /// cFS Software Bus telemetry packet.
    Telemetry,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldDef {
    pub name: String,
    pub optional: bool,
    pub ty: TypeExpr,
    pub default: Option<Literal>,
    pub doc: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeExpr {
    pub base: BaseType,
    pub array: Option<ArraySuffix>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BaseType {
    Primitive(PrimitiveType),
    String,
    Ref(ScopedIdent),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveType {
    F32,
    F64,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    Bool,
    Bytes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArraySuffix {
    /// `[]` — unbounded dynamic (Vec<T>)
    Dynamic,
    /// `[N]` — fixed size ([T; N])
    Fixed(u64),
    /// `[<=N]` — bounded dynamic (Vec<T> with max N)
    Bounded(u64),
}

/// A namespace-qualified identifier, stored as individual segments.
/// `geometry::Point` → `["geometry", "Point"]`
pub type ScopedIdent = Vec<String>;

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Float(f64),
    Int(i64),
    Hex(u64),
    Bool(bool),
    Str(String),
    /// Enum variant or constant reference, e.g. `DriveMode::Idle`
    Ident(ScopedIdent),
}

/// A declaration attribute, e.g. `@cc(1)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Attribute {
    pub name: String,
    pub value: Literal,
}

mod builder;

pub use builder::parse;

#[cfg(test)]
mod tests;
