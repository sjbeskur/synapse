use pest::{error::Error, iterators::Pair, Parser};

use crate::synapse::{Rule, SynapseParser};

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
    pub name: String,
    pub fields: Vec<FieldDef>,
    pub doc: Vec<String>,
    pub attrs: Vec<Attribute>,
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

/// A declaration attribute, e.g. `@mid(0x0801)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Attribute {
    pub name: String,
    pub value: Literal,
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn parse(input: &str) -> Result<SynFile, Error<Rule>> {
    let file_pair = SynapseParser::parse(Rule::file, input)?.next().unwrap();
    Ok(build_file(file_pair))
}

// ── Builders ──────────────────────────────────────────────────────────────────

fn build_file(pair: Pair<Rule>) -> SynFile {
    let items = pair
        .into_inner()
        .filter_map(|p| match p.as_rule() {
            Rule::namespace_decl => Some(Item::Namespace(build_namespace(p))),
            Rule::import_decl    => Some(Item::Import(build_import(p))),
            Rule::const_decl     => Some(Item::Const(build_const(p))),
            Rule::enum_def       => Some(Item::Enum(build_enum(p))),
            Rule::struct_def     => Some(Item::Struct(build_struct(p))),
            Rule::message_def    => Some(Item::Message(build_message(p))),
            Rule::EOI            => None,
            r                    => unreachable!("unexpected rule: {:?}", r),
        })
        .collect();
    SynFile { items }
}

fn build_namespace(pair: Pair<Rule>) -> NamespaceDecl {
    let scoped = pair.into_inner().next().unwrap();
    NamespaceDecl { name: build_scoped_ident(scoped) }
}

fn build_import(pair: Pair<Rule>) -> ImportDecl {
    let s = pair.into_inner().next().unwrap().as_str();
    ImportDecl { path: s[1..s.len() - 1].to_string() }
}

fn build_const(pair: Pair<Rule>) -> ConstDecl {
    let mut inner = pair.into_inner().peekable();
    let doc   = extract_doc(&mut inner);
    let attrs = extract_attrs(&mut inner);
    let name  = inner.next().unwrap().as_str().to_string();
    let ty    = build_type_expr(inner.next().unwrap());
    let value = build_literal(inner.next().unwrap());
    ConstDecl { name, ty, value, doc, attrs }
}

fn build_enum(pair: Pair<Rule>) -> EnumDef {
    let mut inner = pair.into_inner().peekable();
    let doc      = extract_doc(&mut inner);
    let attrs    = extract_attrs(&mut inner);
    let name     = inner.next().unwrap().as_str().to_string();
    let variants = inner.map(build_enum_variant).collect();
    EnumDef { name, variants, doc, attrs }
}

fn build_enum_variant(pair: Pair<Rule>) -> EnumVariant {
    let mut inner = pair.into_inner().peekable();
    let doc   = extract_doc(&mut inner);
    let name  = inner.next().unwrap().as_str().to_string();
    let value = inner.next().map(|p| p.as_str().parse::<i64>().unwrap());
    EnumVariant { name, value, doc }
}

fn build_struct(pair: Pair<Rule>) -> StructDef {
    let mut inner = pair.into_inner().peekable();
    let doc    = extract_doc(&mut inner);
    let attrs  = extract_attrs(&mut inner);
    let name   = inner.next().unwrap().as_str().to_string();
    let fields = inner.map(build_field).collect();
    StructDef { name, fields, doc, attrs }
}

fn build_message(pair: Pair<Rule>) -> MessageDef {
    let mut inner = pair.into_inner().peekable();
    let doc    = extract_doc(&mut inner);
    let attrs  = extract_attrs(&mut inner);
    let name   = inner.next().unwrap().as_str().to_string();
    let fields = inner.map(build_field).collect();
    MessageDef { name, fields, doc, attrs }
}

fn build_field(pair: Pair<Rule>) -> FieldDef {
    let mut inner = pair.into_inner().peekable();
    let doc  = extract_doc(&mut inner);
    let name = inner.next().unwrap().as_str().to_string();

    let next = inner.next().unwrap();
    let (optional, type_pair) = if next.as_rule() == Rule::optional_marker {
        (true, inner.next().unwrap())
    } else {
        (false, next)
    };

    let ty      = build_type_expr(type_pair);
    let default = inner.next().map(build_literal);

    FieldDef { name, optional, ty, default, doc }
}

/// Consume a leading `doc_block` (if present) and return the trimmed doc lines.
fn extract_doc<'i>(
    inner: &mut std::iter::Peekable<impl Iterator<Item = Pair<'i, Rule>>>,
) -> Vec<String> {
    if inner.peek().map(|p| p.as_rule()) == Some(Rule::doc_block) {
        inner
            .next()
            .unwrap()
            .into_inner()
            .map(|p| p.as_str().strip_prefix("##").unwrap_or("").trim().to_string())
            .collect()
    } else {
        vec![]
    }
}

/// Consume zero or more leading `attribute` pairs and return them.
fn extract_attrs<'i>(
    inner: &mut std::iter::Peekable<impl Iterator<Item = Pair<'i, Rule>>>,
) -> Vec<Attribute> {
    let mut attrs = vec![];
    while inner.peek().map(|p| p.as_rule()) == Some(Rule::attribute) {
        let attr = inner.next().unwrap();
        let mut ai = attr.into_inner();
        let name  = ai.next().unwrap().as_str().to_string();
        let value = build_literal(ai.next().unwrap());
        attrs.push(Attribute { name, value });
    }
    attrs
}

fn build_type_expr(pair: Pair<Rule>) -> TypeExpr {
    let mut inner = pair.into_inner();
    let base  = build_base_type(inner.next().unwrap());
    let array = inner.next().map(build_array_suffix);
    TypeExpr { base, array }
}

fn build_base_type(pair: Pair<Rule>) -> BaseType {
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::string_type    => BaseType::String,
        Rule::primitive_type => BaseType::Primitive(build_primitive_type(inner)),
        Rule::type_ref       => BaseType::Ref(build_scoped_ident(
            inner.into_inner().next().unwrap(),
        )),
        r => unreachable!("unexpected base_type rule: {:?}", r),
    }
}

fn build_primitive_type(pair: Pair<Rule>) -> PrimitiveType {
    match pair.as_str() {
        "f32"   => PrimitiveType::F32,
        "f64"   => PrimitiveType::F64,
        "i8"    => PrimitiveType::I8,
        "i16"   => PrimitiveType::I16,
        "i32"   => PrimitiveType::I32,
        "i64"   => PrimitiveType::I64,
        "u8"    => PrimitiveType::U8,
        "u16"   => PrimitiveType::U16,
        "u32"   => PrimitiveType::U32,
        "u64"   => PrimitiveType::U64,
        "bool"  => PrimitiveType::Bool,
        "bytes" => PrimitiveType::Bytes,
        s       => unreachable!("unknown primitive: {}", s),
    }
}

fn build_array_suffix(pair: Pair<Rule>) -> ArraySuffix {
    match pair.into_inner().next() {
        None    => ArraySuffix::Dynamic,
        Some(p) => {
            let inner = p.into_inner().next().unwrap();
            match inner.as_rule() {
                Rule::bounded_size => {
                    let n = inner
                        .into_inner()
                        .next()
                        .unwrap()
                        .as_str()
                        .parse::<u64>()
                        .unwrap();
                    ArraySuffix::Bounded(n)
                }
                Rule::pos_int => ArraySuffix::Fixed(inner.as_str().parse::<u64>().unwrap()),
                r => unreachable!("unexpected array_size rule: {:?}", r),
            }
        }
    }
}

fn build_literal(pair: Pair<Rule>) -> Literal {
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::float_lit  => Literal::Float(inner.as_str().parse::<f64>().unwrap()),
        Rule::hex_lit    => {
            let s = inner.as_str();
            let digits = &s[2..]; // strip 0x / 0X
            Literal::Hex(u64::from_str_radix(digits, 16).unwrap())
        }
        Rule::int_lit    => Literal::Int(inner.as_str().parse::<i64>().unwrap()),
        Rule::bool_lit   => Literal::Bool(inner.as_str() == "true"),
        Rule::string_lit => {
            let s = inner.as_str();
            Literal::Str(unescape(&s[1..s.len() - 1]))
        }
        Rule::ident_lit  => Literal::Ident(build_scoped_ident(
            inner.into_inner().next().unwrap(),
        )),
        r => unreachable!("unexpected literal rule: {:?}", r),
    }
}

fn build_scoped_ident(pair: Pair<Rule>) -> ScopedIdent {
    pair.into_inner().map(|p| p.as_str().to_string()).collect()
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n')  => out.push('\n'),
                Some('t')  => out.push('\t'),
                Some('r')  => out.push('\r'),
                Some('\\') => out.push('\\'),
                Some('"')  => out.push('"'),
                Some(c)    => { out.push('\\'); out.push(c); }
                None       => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn p(input: &str) -> SynFile {
        parse(input).expect("parse failed")
    }

    // ── Namespace ────────────────────────────────────────────

    #[test]
    fn namespace_simple() {
        let f = p("namespace geometry");
        assert_eq!(
            f.items[0],
            Item::Namespace(NamespaceDecl { name: vec!["geometry".into()] })
        );
    }

    #[test]
    fn namespace_qualified() {
        let f = p("namespace nav::msgs");
        assert_eq!(
            f.items[0],
            Item::Namespace(NamespaceDecl {
                name: vec!["nav".into(), "msgs".into()]
            })
        );
    }

    // ── Import ───────────────────────────────────────────────

    #[test]
    fn import_path() {
        let f = p(r#"import "geometry.syn""#);
        assert_eq!(
            f.items[0],
            Item::Import(ImportDecl { path: "geometry.syn".into() })
        );
    }

    // ── Const ────────────────────────────────────────────────

    #[test]
    fn const_float() {
        let f = p("const PI: f64 = 3.14");
        assert_eq!(
            f.items[0],
            Item::Const(ConstDecl {
                name: "PI".into(),
                ty: TypeExpr { base: BaseType::Primitive(PrimitiveType::F64), array: None },
                value: Literal::Float(3.14),
                doc: vec![], attrs: vec![],
            })
        );
    }

    #[test]
    fn const_int() {
        let f = p("const MAX: u32 = 256");
        assert_eq!(
            f.items[0],
            Item::Const(ConstDecl {
                name: "MAX".into(),
                ty: TypeExpr { base: BaseType::Primitive(PrimitiveType::U32), array: None },
                value: Literal::Int(256),
                doc: vec![], attrs: vec![],
            })
        );
    }

    #[test]
    fn const_string() {
        let f = p(r#"const FRAME: string = "world""#);
        assert_eq!(
            f.items[0],
            Item::Const(ConstDecl {
                name: "FRAME".into(),
                ty: TypeExpr { base: BaseType::String, array: None },
                value: Literal::Str("world".into()),
                doc: vec![], attrs: vec![],
            })
        );
    }

    // ── Enum ─────────────────────────────────────────────────

    #[test]
    fn enum_with_values() {
        let f = p("enum DriveMode { Idle = 0  Forward = 1  Error = 2 }");
        let Item::Enum(e) = &f.items[0] else { panic!() };
        assert_eq!(e.name, "DriveMode");
        assert_eq!(e.variants[0], EnumVariant { name: "Idle".into(),    value: Some(0), doc: vec![] });
        assert_eq!(e.variants[1], EnumVariant { name: "Forward".into(), value: Some(1), doc: vec![] });
        assert_eq!(e.variants[2], EnumVariant { name: "Error".into(),   value: Some(2), doc: vec![] });
        assert!(e.attrs.is_empty());
    }

    #[test]
    fn enum_without_values() {
        let f = p("enum Dir { North South East West }");
        let Item::Enum(e) = &f.items[0] else { panic!() };
        assert!(e.variants.iter().all(|v| v.value.is_none()));
        assert_eq!(e.variants.len(), 4);
    }

    // ── Struct ───────────────────────────────────────────────

    #[test]
    fn struct_basic() {
        let f = p("struct Point { x: f64 = 0.0  y: f64 = 0.0  z: f64 = 0.0 }");
        let Item::Struct(s) = &f.items[0] else { panic!() };
        assert_eq!(s.name, "Point");
        assert_eq!(s.fields.len(), 3);
        assert_eq!(s.fields[0].name, "x");
        assert_eq!(s.fields[0].ty.base, BaseType::Primitive(PrimitiveType::F64));
        assert_eq!(s.fields[0].default, Some(Literal::Float(0.0)));
        assert!(!s.fields[0].optional);
    }

    #[test]
    fn struct_qualified_type() {
        let f = p("struct Pose { position: geometry::Point  orientation: geometry::Quaternion }");
        let Item::Struct(s) = &f.items[0] else { panic!() };
        assert_eq!(
            s.fields[0].ty.base,
            BaseType::Ref(vec!["geometry".into(), "Point".into()])
        );
    }

    // ── Message ──────────────────────────────────────────────

    #[test]
    fn message_optional_field() {
        let f = p("message Foo { required: i32  optional?: string }");
        let Item::Message(m) = &f.items[0] else { panic!() };
        assert!(!m.fields[0].optional);
        assert!(m.fields[1].optional);
        assert_eq!(m.fields[1].ty.base, BaseType::String);
    }

    #[test]
    fn message_array_fields() {
        let f = p("message D { dynamic: u8[]  fixed: f64[3]  bounded: u8[<=256] }");
        let Item::Message(m) = &f.items[0] else { panic!() };
        assert_eq!(m.fields[0].ty.array, Some(ArraySuffix::Dynamic));
        assert_eq!(m.fields[1].ty.array, Some(ArraySuffix::Fixed(3)));
        assert_eq!(m.fields[2].ty.array, Some(ArraySuffix::Bounded(256)));
    }

    #[test]
    fn message_enum_default() {
        let f = p("message S { mode: DriveMode = DriveMode::Idle }");
        let Item::Message(m) = &f.items[0] else { panic!() };
        assert_eq!(
            m.fields[0].default,
            Some(Literal::Ident(vec!["DriveMode".into(), "Idle".into()]))
        );
    }

    #[test]
    fn message_string_bounded() {
        let f = p(r#"message S { label: string[<=64] = "robot" }"#);
        let Item::Message(m) = &f.items[0] else { panic!() };
        assert_eq!(m.fields[0].ty.base, BaseType::String);
        assert_eq!(m.fields[0].ty.array, Some(ArraySuffix::Bounded(64)));
        assert_eq!(m.fields[0].default, Some(Literal::Str("robot".into())));
    }

    // ── Hex literal ──────────────────────────────────────────

    #[test]
    fn hex_literal_const() {
        let f = p("const MID: u16 = 0x0801");
        let Item::Const(c) = &f.items[0] else { panic!() };
        assert_eq!(c.value, Literal::Hex(0x0801));
    }

    #[test]
    fn hex_literal_uppercase() {
        let f = p("const MID: u16 = 0X1F80");
        let Item::Const(c) = &f.items[0] else { panic!() };
        assert_eq!(c.value, Literal::Hex(0x1F80));
    }

    // ── Attributes ───────────────────────────────────────────

    #[test]
    fn attribute_hex_on_message() {
        let f = p("@mid(0x0801)\nmessage NavTlm { x: f64 }");
        let Item::Message(m) = &f.items[0] else { panic!() };
        assert_eq!(m.attrs.len(), 1);
        assert_eq!(m.attrs[0].name, "mid");
        assert_eq!(m.attrs[0].value, Literal::Hex(0x0801));
    }

    #[test]
    fn attribute_ident_ref() {
        let f = p("@mid(nav_app::NAV_TLM_MID)\nmessage NavTlm { x: f64 }");
        let Item::Message(m) = &f.items[0] else { panic!() };
        assert_eq!(m.attrs[0].value, Literal::Ident(vec!["nav_app".into(), "NAV_TLM_MID".into()]));
    }

    #[test]
    fn no_attrs_is_empty() {
        let f = p("message Foo { x: i32 }");
        let Item::Message(m) = &f.items[0] else { panic!() };
        assert!(m.attrs.is_empty());
    }

    // ── String escape sequences ───────────────────────────────

    #[test]
    fn string_escape_sequences() {
        let f = p(r#"const S: string = "hello\nworld""#);
        let Item::Const(c) = &f.items[0] else { panic!() };
        assert_eq!(c.value, Literal::Str("hello\nworld".into()));
    }

    #[test]
    fn string_escape_quote() {
        let f = p(r#"const S: string = "say \"hi\"""#);
        let Item::Const(c) = &f.items[0] else { panic!() };
        assert_eq!(c.value, Literal::Str("say \"hi\"".into()));
    }

    // ── Full realistic file ───────────────────────────────────

    #[test]
    fn full_robot_state() {
        let src = r#"
            namespace robot
            import "geometry.syn"

            enum DriveMode {
                Idle    = 0
                Forward = 1
                Error   = 2
            }

            const MAX_SPEED: f64 = 2.5

            message RobotState {
                mode:        DriveMode      = DriveMode::Idle
                position:    geometry::Point
                battery:     f32            = 100.0
                label:       string[<=64]   = "robot"
                sensor_data: u8[]
                error_code?: i32
            }
        "#;

        let f = parse(src).unwrap();
        assert_eq!(f.items.len(), 5);

        let Item::Namespace(ns) = &f.items[0] else { panic!() };
        assert_eq!(ns.name, vec!["robot"]);

        let Item::Enum(e) = &f.items[2] else { panic!() };
        assert_eq!(e.variants.len(), 3);

        let Item::Message(m) = &f.items[4] else { panic!() };
        assert_eq!(m.name, "RobotState");
        assert_eq!(m.fields.len(), 6);

        // last field is optional
        assert!(m.fields[5].optional);
        assert_eq!(m.fields[5].name, "error_code");
    }
}
