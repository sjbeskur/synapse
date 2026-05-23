use pest::{Parser, error::Error, iterators::Pair};

use crate::synapse::{Rule, SynapseParser};

use super::*;

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
            Rule::import_decl => Some(Item::Import(build_import(p))),
            Rule::const_decl => Some(Item::Const(build_const(p))),
            Rule::enum_def => Some(Item::Enum(build_enum(p))),
            Rule::struct_def => Some(Item::Struct(build_struct(p))),
            Rule::table_def => Some(Item::Table(build_struct(p))),
            Rule::command_def => Some(Item::Command(build_packet(p, PacketKind::Command))),
            Rule::telemetry_def => Some(Item::Telemetry(build_packet(p, PacketKind::Telemetry))),
            Rule::message_def => Some(Item::Message(build_packet(p, PacketKind::Message))),
            Rule::EOI => None,
            r => unreachable!("unexpected rule: {:?}", r),
        })
        .collect();
    SynFile { items }
}

fn build_namespace(pair: Pair<Rule>) -> NamespaceDecl {
    let scoped = pair.into_inner().next().unwrap();
    NamespaceDecl {
        name: build_scoped_ident(scoped),
    }
}

fn build_import(pair: Pair<Rule>) -> ImportDecl {
    let s = pair.into_inner().next().unwrap().as_str();
    ImportDecl {
        path: s[1..s.len() - 1].to_string(),
    }
}

fn build_const(pair: Pair<Rule>) -> ConstDecl {
    let mut inner = pair.into_inner().peekable();
    let doc = extract_doc(&mut inner);
    let attrs = extract_attrs(&mut inner);
    let name = inner.next().unwrap().as_str().to_string();
    let ty = build_type_expr(inner.next().unwrap());
    let value = build_literal(inner.next().unwrap());
    ConstDecl {
        name,
        ty,
        value,
        doc,
        attrs,
    }
}

fn build_enum(pair: Pair<Rule>) -> EnumDef {
    let mut inner = pair.into_inner().peekable();
    let doc = extract_doc(&mut inner);
    let attrs = extract_attrs(&mut inner);
    let first = inner.next().unwrap();
    let (repr, name) = if first.as_rule() == Rule::primitive_type {
        let repr = build_primitive_type(first);
        (Some(repr), inner.next().unwrap().as_str().to_string())
    } else {
        (None, first.as_str().to_string())
    };
    let variants = inner.map(build_enum_variant).collect();
    EnumDef {
        name,
        repr,
        variants,
        doc,
        attrs,
    }
}

fn build_enum_variant(pair: Pair<Rule>) -> EnumVariant {
    let mut inner = pair.into_inner().peekable();
    let doc = extract_doc(&mut inner);
    let name = inner.next().unwrap().as_str().to_string();
    let value = inner.next().map(|p| p.as_str().parse::<i64>().unwrap());
    EnumVariant { name, value, doc }
}

fn build_struct(pair: Pair<Rule>) -> StructDef {
    let mut inner = pair.into_inner().peekable();
    let doc = extract_doc(&mut inner);
    let attrs = extract_attrs(&mut inner);
    let name = inner.next().unwrap().as_str().to_string();
    let fields = inner.map(build_field).collect();
    StructDef {
        name,
        fields,
        doc,
        attrs,
    }
}

fn build_packet(pair: Pair<Rule>, kind: PacketKind) -> MessageDef {
    let mut inner = pair.into_inner().peekable();
    let doc = extract_doc(&mut inner);
    let attrs = extract_attrs(&mut inner);
    let name = inner.next().unwrap().as_str().to_string();
    let fields = inner.map(build_field).collect();
    MessageDef {
        kind,
        name,
        fields,
        doc,
        attrs,
    }
}

fn build_field(pair: Pair<Rule>) -> FieldDef {
    let mut inner = pair.into_inner().peekable();
    let doc = extract_doc(&mut inner);
    let name = inner.next().unwrap().as_str().to_string();

    let next = inner.next().unwrap();
    let (optional, type_pair) = if next.as_rule() == Rule::optional_marker {
        (true, inner.next().unwrap())
    } else {
        (false, next)
    };

    let ty = build_type_expr(type_pair);
    let default = inner.next().map(build_literal);

    FieldDef {
        name,
        optional,
        ty,
        default,
        doc,
    }
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
            .map(|p| {
                p.as_str()
                    .strip_prefix("///")
                    .unwrap_or("")
                    .trim()
                    .to_string()
            })
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
        let name = ai.next().unwrap().as_str().to_string();
        let value = build_literal(ai.next().unwrap());
        attrs.push(Attribute { name, value });
    }
    attrs
}

fn build_type_expr(pair: Pair<Rule>) -> TypeExpr {
    let mut inner = pair.into_inner();
    let base = build_base_type(inner.next().unwrap());
    let array = inner.next().map(build_array_suffix);
    TypeExpr { base, array }
}

fn build_base_type(pair: Pair<Rule>) -> BaseType {
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::string_type => BaseType::String,
        Rule::primitive_type => BaseType::Primitive(build_primitive_type(inner)),
        Rule::type_ref => BaseType::Ref(build_scoped_ident(inner.into_inner().next().unwrap())),
        r => unreachable!("unexpected base_type rule: {:?}", r),
    }
}

fn build_primitive_type(pair: Pair<Rule>) -> PrimitiveType {
    const PRIMITIVES: &[(&str, PrimitiveType)] = &[
        ("f32", PrimitiveType::F32),
        ("f64", PrimitiveType::F64),
        ("i8", PrimitiveType::I8),
        ("i16", PrimitiveType::I16),
        ("i32", PrimitiveType::I32),
        ("i64", PrimitiveType::I64),
        ("u8", PrimitiveType::U8),
        ("u16", PrimitiveType::U16),
        ("u32", PrimitiveType::U32),
        ("u64", PrimitiveType::U64),
        ("bool", PrimitiveType::Bool),
        ("bytes", PrimitiveType::Bytes),
    ];

    let primitive = pair.as_str();
    PRIMITIVES
        .iter()
        .find_map(|(name, ty)| (*name == primitive).then_some(*ty))
        .unwrap_or_else(|| unreachable!("unknown primitive: {}", primitive))
}

fn build_array_suffix(pair: Pair<Rule>) -> ArraySuffix {
    match pair.into_inner().next() {
        None => ArraySuffix::Dynamic,
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
        Rule::float_lit | Rule::hex_lit | Rule::int_lit => build_numeric_literal(inner),
        Rule::bool_lit => build_bool_literal(inner),
        Rule::string_lit => build_string_literal(inner),
        Rule::ident_lit => Literal::Ident(build_scoped_ident(inner.into_inner().next().unwrap())),
        r => unreachable!("unexpected literal rule: {:?}", r),
    }
}

fn build_numeric_literal(pair: Pair<Rule>) -> Literal {
    match pair.as_rule() {
        Rule::float_lit => Literal::Float(pair.as_str().parse::<f64>().unwrap()),
        Rule::hex_lit => {
            let s = pair.as_str();
            let digits = &s[2..]; // strip 0x / 0X
            Literal::Hex(u64::from_str_radix(digits, 16).unwrap())
        }
        Rule::int_lit => Literal::Int(pair.as_str().parse::<i64>().unwrap()),
        r => unreachable!("unexpected numeric literal rule: {:?}", r),
    }
}

fn build_bool_literal(pair: Pair<Rule>) -> Literal {
    Literal::Bool(pair.as_str() == "true")
}

fn build_string_literal(pair: Pair<Rule>) -> Literal {
    let s = pair.as_str();
    Literal::Str(unescape(&s[1..s.len() - 1]))
}

fn build_scoped_ident(pair: Pair<Rule>) -> ScopedIdent {
    pair.into_inner().map(|p| p.as_str().to_string()).collect()
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            push_escape(&mut out, chars.next());
        } else {
            out.push(c);
        }
    }
    out
}

fn push_escape(out: &mut String, escaped: Option<char>) {
    match escaped {
        Some('n') => out.push('\n'),
        Some('t') => out.push('\t'),
        Some('r') => out.push('\r'),
        Some(c) => push_quoted_escape(out, c),
        None => out.push('\\'),
    }
}

fn push_quoted_escape(out: &mut String, escaped: char) {
    match escaped {
        '\\' => out.push('\\'),
        '"' => out.push('"'),
        c => {
            out.push('\\');
            out.push(c);
        }
    }
}
