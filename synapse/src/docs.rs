use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use synapse_parser::ast::{
    ArraySuffix, BaseType, FieldDef, Item, Literal, MessageDef, PacketKind, PrimitiveType,
    StructDef, SynFile, TypeExpr,
};

use crate::{
    CfsOptions, Error, ImportGraph, ParsedUnit, format_mid, imported_constants_for_unit, namespace,
};

const DOC_PAGE_TEMPLATE: &str = include_str!("templates/docs/page.html");
const DOC_STYLE: &str = include_str!("templates/docs/style.css");
const DOC_SEARCH_SCRIPT: &str = include_str!("templates/docs/search.js");

#[derive(Default)]
struct DocSummary {
    commands: usize,
    telemetry: usize,
    structs: usize,
    tables: usize,
    enums: usize,
    constants: usize,
}

pub(crate) fn render_html_docs(
    graph: &ImportGraph,
    units_by_path: &HashMap<PathBuf, &ParsedUnit>,
    options: &CfsOptions,
) -> Result<String, Error> {
    let context = DocContext::new(graph);
    let summary = doc_summary(graph);
    let nav = doc_nav(graph, &context);
    let mut summary_html = String::new();
    render_metric(&mut summary_html, graph.units.len(), "files");
    render_metric(&mut summary_html, summary.telemetry, "telemetry");
    render_metric(&mut summary_html, summary.commands, "commands");
    render_metric(&mut summary_html, summary.structs, "structs");
    render_metric(&mut summary_html, summary.tables, "tables");
    render_metric(&mut summary_html, summary.enums, "enums");
    render_metric(&mut summary_html, summary.constants, "constants");

    let mut toc_html = String::new();
    render_toc(&mut toc_html, &nav);

    let mut content_html = String::new();

    for unit in &graph.units {
        render_doc_unit(&mut content_html, unit, units_by_path, &context, options)?;
    }

    Ok(render_page_template(
        &summary_html,
        &toc_html,
        &content_html,
    ))
}

struct DocContext {
    source_root: PathBuf,
}

impl DocContext {
    fn new(graph: &ImportGraph) -> Self {
        let source_root = common_source_root(graph).unwrap_or_default();
        Self { source_root }
    }

    fn source_label(&self, path: &Path) -> String {
        path.strip_prefix(&self.source_root)
            .unwrap_or(path)
            .display()
            .to_string()
    }
}

fn common_source_root(graph: &ImportGraph) -> Option<PathBuf> {
    let mut paths = graph.units.iter().map(|unit| unit.path.parent());
    let first = paths.next()??;
    let mut root = first.to_path_buf();
    for path in paths.flatten() {
        while !path.starts_with(&root) {
            if !root.pop() {
                return Some(PathBuf::new());
            }
        }
    }
    Some(root)
}

fn render_page_template(summary_html: &str, toc_html: &str, content_html: &str) -> String {
    DOC_PAGE_TEMPLATE
        .replace("{{style}}", DOC_STYLE)
        .replace("{{script}}", DOC_SEARCH_SCRIPT)
        .replace("{{summary}}", summary_html)
        .replace("{{toc}}", toc_html)
        .replace("{{content}}", content_html)
}

fn doc_summary(graph: &ImportGraph) -> DocSummary {
    let mut summary = DocSummary::default();
    for unit in &graph.units {
        for item in &unit.file.items {
            summary.add_item(item);
        }
    }
    summary
}

impl DocSummary {
    fn add_item(&mut self, item: &Item) {
        if let Some(counter) = self.item_counter(item) {
            *counter += 1;
        }
    }

    fn item_counter(&mut self, item: &Item) -> Option<&mut usize> {
        match item {
            Item::Command(_) | Item::Telemetry(_) | Item::Message(_) => self.packet_counter(item),
            Item::Struct(_) | Item::Table(_) | Item::Enum(_) | Item::Const(_) => {
                self.plain_item_counter(item)
            }
            Item::Namespace(_) | Item::Import(_) => None,
        }
    }

    fn packet_counter(&mut self, item: &Item) -> Option<&mut usize> {
        match item {
            Item::Command(_) => Some(&mut self.commands),
            Item::Telemetry(_) => Some(&mut self.telemetry),
            Item::Message(_) => None,
            _ => unreachable!("non-packet item passed to packet_counter"),
        }
    }

    fn plain_item_counter(&mut self, item: &Item) -> Option<&mut usize> {
        match item {
            Item::Struct(_) => Some(&mut self.structs),
            Item::Table(_) => Some(&mut self.tables),
            Item::Enum(_) => Some(&mut self.enums),
            Item::Const(_) => Some(&mut self.constants),
            _ => unreachable!("non-plain item passed to plain_item_counter"),
        }
    }
}

fn render_metric(out: &mut String, value: usize, label: &str) {
    out.push_str(&format!(
        "<div class=\"metric\"><strong>{value}</strong><span>{}</span></div>\n",
        escape_html(label)
    ));
}

struct DocNavEntry {
    id: String,
    label: String,
    kind: String,
}

fn doc_nav(graph: &ImportGraph, context: &DocContext) -> Vec<DocNavEntry> {
    let mut entries = Vec::new();
    for unit in &graph.units {
        let namespace = namespace(&unit.file);
        let namespace_label = namespace_label(&namespace);
        entries.push(DocNavEntry {
            id: unit_id(unit, &namespace_label, context),
            label: namespace_label.clone(),
            kind: "file".to_string(),
        });
        for item in &unit.file.items {
            if let Some((kind, name)) = item_kind_name(item) {
                entries.push(DocNavEntry {
                    id: item_id(unit, kind, name, context),
                    label: format!("{namespace_label}::{name}"),
                    kind: kind.to_string(),
                });
            }
        }
    }
    entries
}

fn render_toc(out: &mut String, entries: &[DocNavEntry]) {
    out.push_str("<nav class=\"toc\" aria-label=\"Documentation contents\">\n");
    for entry in entries {
        out.push_str(&format!(
            "<a href=\"#{}\"><span class=\"kind\">{}</span>{}</a>\n",
            escape_attr(&entry.id),
            escape_html(&entry.kind),
            escape_html(&entry.label)
        ));
    }
    out.push_str("</nav>\n");
}

fn render_doc_unit(
    out: &mut String,
    unit: &ParsedUnit,
    units_by_path: &HashMap<PathBuf, &ParsedUnit>,
    context: &DocContext,
    options: &CfsOptions,
) -> Result<(), Error> {
    let namespace = namespace(&unit.file);
    let namespace_label = namespace_label(&namespace);
    let packet_facts = packet_facts_for_unit(unit, units_by_path, options)?;
    let unit_id = unit_id(unit, &namespace_label, context);
    let unit_search = unit_search_text(unit, &namespace_label, context);

    out.push_str(&format!(
        "<section id=\"{}\" class=\"unit\" data-search=\"{}\">\n",
        escape_attr(&unit_id),
        escape_attr(&unit_search)
    ));
    out.push_str("<div class=\"unit-header\">\n");
    out.push_str(&format!("<h2>{}</h2>\n", escape_html(&namespace_label)));
    out.push_str("<a class=\"top-link\" href=\"#top\">Back to top</a>\n");
    out.push_str("</div>\n");
    out.push_str("<p class=\"meta\">source: ");
    render_source_label(out, unit, context);
    out.push_str("</p>\n");
    render_imports(out, &unit.file);
    out.push_str("<div class=\"items\">\n");

    let mut rendered = 0usize;
    for item in &unit.file.items {
        if render_doc_item(out, unit, item, &packet_facts, context) {
            rendered += 1;
        }
    }

    if rendered == 0 {
        out.push_str("<p class=\"empty\">No documented declarations.</p>\n");
    }
    out.push_str("</div>\n</section>\n");
    Ok(())
}

fn render_doc_item(
    out: &mut String,
    unit: &ParsedUnit,
    item: &Item,
    packet_facts: &HashMap<String, synapse_codegen_cfs::CfsPacket>,
    context: &DocContext,
) -> bool {
    match item {
        Item::Const(_) | Item::Enum(_) | Item::Struct(_) | Item::Table(_) => {
            render_plain_doc_item(out, unit, item, context)
        }
        Item::Command(m) | Item::Telemetry(m) | Item::Message(m) => {
            render_packet_doc(out, unit, m, packet_facts, context)
        }
        Item::Namespace(_) | Item::Import(_) => return false,
    }
    true
}

fn render_plain_doc_item(out: &mut String, unit: &ParsedUnit, item: &Item, context: &DocContext) {
    match item {
        Item::Const(c) => render_const_doc(out, unit, c, context),
        Item::Enum(e) => render_enum_doc(out, unit, e, context),
        Item::Struct(s) => render_struct_doc(out, unit, "struct", s, context),
        Item::Table(s) => render_struct_doc(out, unit, "table", s, context),
        _ => unreachable!("non-plain item passed to render_plain_doc_item"),
    }
}

fn render_const_doc(
    out: &mut String,
    unit: &ParsedUnit,
    c: &synapse_parser::ast::ConstDecl,
    context: &DocContext,
) {
    render_item_open(out, unit, "const", &c.name, &const_search_text(c), context);
    render_item_title(out, unit, "const", &c.name, context);
    render_doc_lines_html(out, &c.doc);
    out.push_str(&format!(
        "<dl><dt>Type</dt><dd><code>{}</code></dd><dt>Value</dt><dd><code>{}</code></dd></dl>\n",
        escape_html(&type_expr_display(&c.ty)),
        escape_html(&literal_display(&c.value))
    ));
    out.push_str("</article>\n");
}

fn render_enum_doc(
    out: &mut String,
    unit: &ParsedUnit,
    e: &synapse_parser::ast::EnumDef,
    context: &DocContext,
) {
    render_item_open(out, unit, "enum", &e.name, &enum_search_text(e), context);
    render_item_title(out, unit, "enum", &e.name, context);
    render_doc_lines_html(out, &e.doc);
    render_enum_repr(out, e.repr);
    render_enum_variants(out, e);
    out.push_str("</article>\n");
}

fn render_enum_repr(out: &mut String, repr: Option<PrimitiveType>) {
    if let Some(repr) = repr {
        out.push_str(&format!(
            "<dl><dt>Representation</dt><dd><code>{}</code></dd></dl>\n",
            escape_html(primitive_name(repr))
        ));
    }
}

fn render_enum_variants(out: &mut String, e: &synapse_parser::ast::EnumDef) {
    out.push_str(
        "<table><thead><tr><th>Variant</th><th>Value</th><th>Description</th></tr></thead><tbody>\n",
    );
    for variant in &e.variants {
        let value = variant
            .value
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".to_string());
        out.push_str(&format!(
            "<tr><td><code>{}</code></td><td><code>{}</code></td><td>{}</td></tr>\n",
            escape_html(&variant.name),
            escape_html(&value),
            escape_html(&variant.doc.join(" "))
        ));
    }
    out.push_str("</tbody></table>\n");
}

fn packet_facts_for_unit(
    unit: &ParsedUnit,
    units_by_path: &HashMap<PathBuf, &ParsedUnit>,
    options: &CfsOptions,
) -> Result<HashMap<String, synapse_codegen_cfs::CfsPacket>, Error> {
    let imported_constants = imported_constants_for_unit(unit, units_by_path)?;
    let packets = synapse_codegen_cfs::collect_cfs_packets_with_constants_and_options(
        &unit.file,
        &imported_constants,
        options,
    )?;
    Ok(packets
        .into_iter()
        .map(|packet| (packet_fact_key(&packet.name, packet.kind), packet))
        .collect())
}

fn render_imports(out: &mut String, file: &SynFile) {
    let imports = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Import(import) => Some(import.path.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    if imports.is_empty() {
        return;
    }

    out.push_str("<p class=\"meta\">imports:</p>\n<ul>\n");
    for import in imports {
        out.push_str(&format!("<li><code>{}</code></li>\n", escape_html(import)));
    }
    out.push_str("</ul>\n");
}

fn render_struct_doc(
    out: &mut String,
    unit: &ParsedUnit,
    kind: &str,
    s: &StructDef,
    context: &DocContext,
) {
    render_item_open(
        out,
        unit,
        kind,
        &s.name,
        &struct_search_text(kind, s),
        context,
    );
    render_item_title(out, unit, kind, &s.name, context);
    render_doc_lines_html(out, &s.doc);
    render_fields(out, &s.fields);
    out.push_str("</article>\n");
}

fn render_packet_doc(
    out: &mut String,
    unit: &ParsedUnit,
    packet: &MessageDef,
    packet_facts: &HashMap<String, synapse_codegen_cfs::CfsPacket>,
    context: &DocContext,
) {
    let kind = packet_kind_label(packet.kind);
    let fact = cfs_packet_fact_key(packet).and_then(|key| packet_facts.get(&key));
    render_item_open(
        out,
        unit,
        kind,
        &packet.name,
        &packet_search_text(packet, fact),
        context,
    );
    render_item_title(out, unit, kind, &packet.name, context);
    render_doc_lines_html(out, &packet.doc);
    if let Some(fact) = fact {
        out.push_str("<dl>");
        out.push_str(&format!(
            "<dt>Topic</dt><dd><code>{}</code></dd>",
            escape_html(&fact.topic)
        ));
        if let Some(mid) = fact.mid {
            out.push_str(&format!(
                "<dt>MID</dt><dd><code>{}</code></dd>",
                escape_html(&format_mid(mid))
            ));
        }
        if let Some(cc) = fact.cc {
            out.push_str(&format!("<dt>CC</dt><dd><code>{cc}</code></dd>"));
        }
        out.push_str("</dl>\n");
    }
    render_fields(out, &packet.fields);
    out.push_str("</article>\n");
}

fn render_item_title(
    out: &mut String,
    unit: &ParsedUnit,
    kind: &str,
    name: &str,
    context: &DocContext,
) {
    out.push_str("<div class=\"item-title\">\n");
    out.push_str(&format!(
        "<h3><span class=\"kind\">{}</span>{}</h3>\n",
        escape_html(kind),
        escape_html(name)
    ));
    out.push_str("<a class=\"source-link\" href=\"");
    out.push_str(&escape_attr(&format!(
        "#{}",
        unit_id(unit, &namespace_label(&namespace(&unit.file)), context)
    )));
    out.push_str("\">Source</a>\n");
    out.push_str("</div>\n");
}

fn render_fields(out: &mut String, fields: &[FieldDef]) {
    if fields.is_empty() {
        out.push_str("<p class=\"empty\">No fields.</p>\n");
        return;
    }

    out.push_str(
        "<table><thead><tr><th>Field</th><th>Type</th><th>Description</th></tr></thead><tbody>\n",
    );
    for field in fields {
        out.push_str(&format!(
            "<tr><td><code>{}</code></td><td><code>{}</code></td><td>{}</td></tr>\n",
            escape_html(&field.name),
            escape_html(&type_expr_display(&field.ty)),
            escape_html(&field.doc.join(" "))
        ));
    }
    out.push_str("</tbody></table>\n");
}

fn render_doc_lines_html(out: &mut String, doc: &[String]) {
    if doc.is_empty() {
        return;
    }
    out.push_str("<p class=\"doc\">");
    for (idx, line) in doc.iter().enumerate() {
        if idx > 0 {
            out.push_str("<br>");
        }
        out.push_str(&escape_html(line));
    }
    out.push_str("</p>\n");
}

fn render_item_open(
    out: &mut String,
    unit: &ParsedUnit,
    kind: &str,
    name: &str,
    search: &str,
    context: &DocContext,
) {
    out.push_str(&format!(
        "<article id=\"{}\" class=\"item\" data-search=\"{}\">\n",
        escape_attr(&item_id(unit, kind, name, context)),
        escape_attr(search)
    ));
}

fn render_source_label(out: &mut String, unit: &ParsedUnit, context: &DocContext) {
    out.push_str("<code>");
    out.push_str(&escape_html(&context.source_label(&unit.path)));
    out.push_str("</code>");
}

fn item_kind_name(item: &Item) -> Option<(&'static str, &str)> {
    let kind = item_kind(item)?;
    let name = item_name(item)?;
    Some((kind, name))
}

fn item_kind(item: &Item) -> Option<&'static str> {
    match item {
        Item::Const(_) | Item::Enum(_) | Item::Struct(_) | Item::Table(_) => plain_item_kind(item),
        Item::Command(_) | Item::Telemetry(_) | Item::Message(_) => packet_item_kind(item),
        Item::Namespace(_) | Item::Import(_) => None,
    }
}

fn plain_item_kind(item: &Item) -> Option<&'static str> {
    match item {
        Item::Const(_) => Some("const"),
        Item::Enum(_) => Some("enum"),
        Item::Struct(_) => Some("struct"),
        Item::Table(_) => Some("table"),
        _ => None,
    }
}

fn packet_item_kind(item: &Item) -> Option<&'static str> {
    match item {
        Item::Command(_) => Some("command"),
        Item::Telemetry(_) => Some("telemetry"),
        Item::Message(_) => Some("message"),
        _ => None,
    }
}

fn item_name(item: &Item) -> Option<&str> {
    match item {
        Item::Const(c) => Some(&c.name),
        Item::Enum(e) => Some(&e.name),
        Item::Struct(s) | Item::Table(s) => Some(&s.name),
        Item::Command(m) | Item::Telemetry(m) | Item::Message(m) => Some(&m.name),
        Item::Namespace(_) | Item::Import(_) => None,
    }
}

fn namespace_label(namespace: &[String]) -> String {
    if namespace.is_empty() {
        "(none)".to_string()
    } else {
        namespace.join("::")
    }
}

fn unit_id(unit: &ParsedUnit, namespace_label: &str, context: &DocContext) -> String {
    slug(&format!(
        "file-{}-{namespace_label}",
        context.source_label(&unit.path)
    ))
}

fn item_id(unit: &ParsedUnit, kind: &str, name: &str, context: &DocContext) -> String {
    slug(&format!(
        "{}-{kind}-{name}",
        context.source_label(&unit.path)
    ))
}

fn unit_search_text(unit: &ParsedUnit, namespace_label: &str, context: &DocContext) -> String {
    let mut parts = vec![
        namespace_label.to_string(),
        context.source_label(&unit.path),
    ];
    for item in &unit.file.items {
        if let Some((kind, name)) = item_kind_name(item) {
            parts.push(kind.to_string());
            parts.push(name.to_string());
        }
    }
    parts.join(" ")
}

fn const_search_text(c: &synapse_parser::ast::ConstDecl) -> String {
    [
        "const".to_string(),
        c.name.clone(),
        type_expr_display(&c.ty),
        literal_display(&c.value),
        c.doc.join(" "),
    ]
    .join(" ")
}

fn enum_search_text(e: &synapse_parser::ast::EnumDef) -> String {
    let mut parts = vec!["enum".to_string(), e.name.clone(), e.doc.join(" ")];
    if let Some(repr) = e.repr {
        parts.push(primitive_name(repr).to_string());
    }
    for variant in &e.variants {
        parts.push(variant.name.clone());
        if let Some(value) = variant.value {
            parts.push(value.to_string());
        }
        parts.push(variant.doc.join(" "));
    }
    parts.join(" ")
}

fn struct_search_text(kind: &str, s: &StructDef) -> String {
    let mut parts = vec![kind.to_string(), s.name.clone(), s.doc.join(" ")];
    for field in &s.fields {
        parts.push(field.name.clone());
        parts.push(type_expr_display(&field.ty));
        parts.push(field.doc.join(" "));
    }
    parts.join(" ")
}

fn packet_search_text(
    packet: &MessageDef,
    fact: Option<&synapse_codegen_cfs::CfsPacket>,
) -> String {
    let mut parts = vec![
        packet_kind_label(packet.kind).to_string(),
        packet.name.clone(),
        packet.doc.join(" "),
    ];
    if let Some(fact) = fact {
        parts.push(fact.topic.clone());
        if let Some(mid) = fact.mid {
            parts.push(format_mid(mid));
            parts.push(mid.to_string());
        }
        if let Some(cc) = fact.cc {
            parts.push(cc.to_string());
        }
    }
    for field in &packet.fields {
        parts.push(field.name.clone());
        parts.push(type_expr_display(&field.ty));
        parts.push(field.doc.join(" "));
    }
    parts.join(" ")
}

fn cfs_packet_fact_key(packet: &MessageDef) -> Option<String> {
    let kind = match packet.kind {
        PacketKind::Command => synapse_codegen_cfs::CfsPacketKind::Command,
        PacketKind::Telemetry => synapse_codegen_cfs::CfsPacketKind::Telemetry,
        PacketKind::Message => return None,
    };
    Some(packet_fact_key(&packet.name, kind))
}

fn packet_fact_key(name: &str, kind: synapse_codegen_cfs::CfsPacketKind) -> String {
    let kind = match kind {
        synapse_codegen_cfs::CfsPacketKind::Command => "command",
        synapse_codegen_cfs::CfsPacketKind::Telemetry => "telemetry",
    };
    format!("{kind}:{name}")
}

fn packet_kind_label(kind: PacketKind) -> &'static str {
    match kind {
        PacketKind::Command => "command",
        PacketKind::Telemetry => "telemetry",
        PacketKind::Message => "message",
    }
}

fn type_expr_display(ty: &TypeExpr) -> String {
    let mut out = base_type_display(&ty.base);
    match &ty.array {
        None => {}
        Some(ArraySuffix::Dynamic) => out.push_str("[]"),
        Some(ArraySuffix::Fixed(n)) => out.push_str(&format!("[{n}]")),
        Some(ArraySuffix::Bounded(n)) => out.push_str(&format!("[<={n}]")),
    }
    out
}

fn base_type_display(base: &BaseType) -> String {
    match base {
        BaseType::Primitive(p) => primitive_name(*p).to_string(),
        BaseType::String => "string".to_string(),
        BaseType::Ref(segments) => segments.join("::"),
    }
}

fn primitive_name(p: PrimitiveType) -> &'static str {
    const NAMES: &[(PrimitiveType, &str)] = &[
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
        (PrimitiveType::Bytes, "bytes"),
    ];

    NAMES
        .iter()
        .find_map(|(ty, name)| (*ty == p).then_some(*name))
        .expect("all primitive types have doc names")
}

fn literal_display(lit: &Literal) -> String {
    match lit {
        Literal::Float(_) | Literal::Int(_) | Literal::Hex(_) => numeric_literal_display(lit),
        Literal::Bool(b) => bool_literal_display(*b),
        Literal::Str(s) => format!("\"{s}\""),
        Literal::Ident(segments) => segments.join("::"),
    }
}

fn numeric_literal_display(lit: &Literal) -> String {
    match lit {
        Literal::Float(f) => f.to_string(),
        Literal::Int(n) => n.to_string(),
        Literal::Hex(n) => format!("0x{n:X}"),
        _ => unreachable!("non-numeric literal passed to numeric_literal_display"),
    }
}

fn bool_literal_display(value: bool) -> String {
    value.to_string()
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn escape_attr(value: &str) -> String {
    escape_html(value)
}

fn slug(value: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "item".to_string()
    } else {
        out
    }
}
