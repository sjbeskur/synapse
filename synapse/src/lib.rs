mod docs;
mod errors;
mod registry;

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use synapse_parser::ast::{BaseType, FieldDef, Item, SynFile};

pub use errors::Error;

/// Target language for Synapse code generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// NASA cFS C header (`.h`).
    C,
    /// Rust `#[repr(C)]` bindings (`.rs`).
    Rust,
}

/// Machine-readable packet registry output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryFormat {
    /// JSON object with a `packets` array.
    Json,
    /// CSV table with one packet per row.
    Csv,
}

impl Lang {
    /// File extension used for this generated language.
    pub fn extension(self) -> &'static str {
        match self {
            Lang::C => "h",
            Lang::Rust => "rs",
        }
    }
}

/// Generate code from `.syn` source text.
pub fn generate_str(source: &str, lang: Lang) -> Result<String, Error> {
    let file = synapse_parser::ast::parse(source)?;
    generate_parsed(&file, lang)
}

/// Check `.syn` source text for parser and cFS codegen support.
pub fn check_str(source: &str) -> Result<(), Error> {
    let file = synapse_parser::ast::parse(source)?;
    synapse_codegen_cfs::validate_cfs(&file)?;
    Ok(())
}

/// Check a `.syn` input path, validating its import graph and cFS codegen support.
pub fn check_path(input: impl AsRef<Path>) -> Result<(), Error> {
    check_paths([input.as_ref()])
}

/// Check one or more `.syn` input paths as a mission-visible set.
///
/// Each root and its imports are validated normally. When more than one root is
/// supplied, Synapse also validates mission-wide packet ID uniqueness across the
/// deduplicated import closure.
pub fn check_paths<I, P>(inputs: I) -> Result<(), Error>
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let inputs = collect_input_paths(inputs, "check")?;
    let graph = load_import_graphs(&inputs)?;
    validate_cfs_graph(&graph)?;
    Ok(())
}

/// Generate static HTML documentation from one or more `.syn` input paths.
pub fn generate_docs<I, P>(inputs: I) -> Result<String, Error>
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let inputs = collect_input_paths(inputs, "doc")?;
    let graph = load_import_graphs(&inputs)?;
    validate_cfs_graph(&graph)?;
    let units_by_path = units_by_path(&graph);

    docs::render_html_docs(&graph, &units_by_path)
}

/// Generate static HTML documentation and write it to `out_dir/index.html`.
pub fn write_docs<I, P>(inputs: I, out_dir: impl AsRef<Path>) -> Result<PathBuf, Error>
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let output = generate_docs(inputs)?;
    let out_dir = out_dir.as_ref();
    fs::create_dir_all(out_dir)?;
    let out_path = out_dir.join("index.html");
    fs::write(&out_path, output)?;
    Ok(out_path)
}

/// Generate a machine-readable packet registry from one or more `.syn` input paths.
pub fn generate_registry<I, P>(inputs: I, format: RegistryFormat) -> Result<String, Error>
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let inputs = collect_input_paths(inputs, "registry")?;
    let graph = load_import_graphs(&inputs)?;
    validate_cfs_graph(&graph)?;
    let units_by_path = units_by_path(&graph);

    registry::render_registry(&graph, &units_by_path, format)
}

/// Generate a machine-readable packet registry and write it to `output`.
pub fn write_registry<I, P>(
    inputs: I,
    output: impl AsRef<Path>,
    format: RegistryFormat,
) -> Result<PathBuf, Error>
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let registry = generate_registry(inputs, format)?;
    let output = output.as_ref();
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    fs::write(output, registry)?;
    Ok(output.to_path_buf())
}

/// Generate code from a `.syn` input path, validating the import graph rooted at that file.
pub fn generate_path(input: impl AsRef<Path>, lang: Lang) -> Result<String, Error> {
    let graph = load_import_graph(input.as_ref())?;
    validate_import_graph(&graph)?;
    let units_by_path = units_by_path(&graph);
    let root = graph
        .units
        .last()
        .expect("import graph always contains the root input");
    let imported_constants = imported_constants_for_unit(root, &units_by_path)?;
    generate_parsed_with_constants(&root.file, lang, &imported_constants)
}

fn generate_parsed(file: &SynFile, lang: Lang) -> Result<String, Error> {
    generate_parsed_with_constants(file, lang, &synapse_codegen_cfs::ResolvedConstants::new())
}

fn collect_input_paths<I, P>(inputs: I, command: &str) -> Result<Vec<PathBuf>, Error>
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let inputs = inputs
        .into_iter()
        .map(|input| input.as_ref().to_path_buf())
        .collect::<Vec<_>>();
    if inputs.is_empty() {
        return Err(Error::Mission(format!(
            "{command} requires at least one input .syn file"
        )));
    }
    Ok(inputs)
}

fn validate_cfs_graph(graph: &ImportGraph) -> Result<(), Error> {
    validate_import_graph(graph)?;
    let units_by_path = units_by_path(graph);
    validate_cfs_units(graph, &units_by_path)?;
    validate_mission_registry(graph, &units_by_path)
}

fn validate_cfs_units(
    graph: &ImportGraph,
    units_by_path: &HashMap<PathBuf, &ParsedUnit>,
) -> Result<(), Error> {
    for unit in &graph.units {
        let imported_constants = imported_constants_for_unit(unit, units_by_path)?;
        synapse_codegen_cfs::validate_cfs_with_constants(&unit.file, &imported_constants)?;
    }
    Ok(())
}

fn generate_parsed_with_constants(
    file: &SynFile,
    lang: Lang,
    imported_constants: &synapse_codegen_cfs::ResolvedConstants,
) -> Result<String, Error> {
    let output = match lang {
        Lang::C => synapse_codegen_cfs::try_generate_c_with_constants(file, imported_constants)?,
        Lang::Rust => synapse_codegen_cfs::try_generate_rust_with_constants(
            file,
            &Default::default(),
            imported_constants,
        )?,
    };
    Ok(output)
}

/// Generate code from an input file and write it into `out_dir`.
///
/// The output file uses the input file stem plus the target language extension,
/// for example `my_msgs.syn` becomes `my_msgs.h` for [`Lang::C`].
pub fn generate_file(
    input: impl AsRef<Path>,
    out_dir: impl AsRef<Path>,
    lang: Lang,
) -> Result<PathBuf, Error> {
    let input = input.as_ref();
    let output = generate_path(input, lang)?;

    let out_dir = out_dir.as_ref();
    fs::create_dir_all(out_dir)?;

    let stem = input.file_stem().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("input file has no stem: {}", input.display()),
        )
    })?;
    let out_path = out_dir.join(format!("{}.{}", stem.to_string_lossy(), lang.extension()));
    fs::write(&out_path, output)?;
    Ok(out_path)
}

/// Generate the root file and all transitive imports into `out_dir`.
///
/// Files are written in dependency order, so imported files appear before files
/// that depend on them in the returned path list. Output filenames use each
/// input file stem plus the target language extension.
pub fn generate_files(
    input: impl AsRef<Path>,
    out_dir: impl AsRef<Path>,
    lang: Lang,
) -> Result<Vec<PathBuf>, Error> {
    let graph = load_import_graph(input.as_ref())?;
    validate_import_graph(&graph)?;
    let units_by_path = units_by_path(&graph);

    let out_dir = out_dir.as_ref();
    fs::create_dir_all(out_dir)?;

    let mut written = Vec::new();
    for unit in &graph.units {
        written.push(write_generated_unit(unit, &units_by_path, out_dir, lang)?);
    }
    Ok(written)
}

fn write_generated_unit(
    unit: &ParsedUnit,
    units_by_path: &HashMap<PathBuf, &ParsedUnit>,
    out_dir: &Path,
    lang: Lang,
) -> Result<PathBuf, Error> {
    let imported_constants = imported_constants_for_unit(unit, units_by_path)?;
    let output = generate_parsed_with_constants(&unit.file, lang, &imported_constants)?;
    let out_path = output_path_for(&unit.path, out_dir, lang)?;
    fs::write(&out_path, output)?;
    Ok(out_path)
}

#[derive(Debug)]
struct ImportGraph {
    units: Vec<ParsedUnit>,
}

#[derive(Debug)]
struct ParsedUnit {
    path: PathBuf,
    file: SynFile,
}

fn load_import_graph(input: &Path) -> Result<ImportGraph, Error> {
    load_import_graphs(&[input.to_path_buf()])
}

fn load_import_graphs(inputs: &[PathBuf]) -> Result<ImportGraph, Error> {
    let mut units = Vec::new();
    let mut visited = HashSet::new();
    let mut visiting = HashSet::new();
    let mut stack = Vec::new();
    for input in inputs {
        load_import_unit(input, &mut units, &mut visited, &mut visiting, &mut stack)?;
    }
    Ok(ImportGraph { units })
}

fn load_import_unit(
    input: &Path,
    units: &mut Vec<ParsedUnit>,
    visited: &mut HashSet<PathBuf>,
    visiting: &mut HashSet<PathBuf>,
    stack: &mut Vec<PathBuf>,
) -> Result<(), Error> {
    let path = canonicalize_import_path(input)?;
    if visited.contains(&path) {
        return Ok(());
    }
    reject_import_cycle(&path, visiting, stack)?;

    visiting.insert(path.clone());
    stack.push(path.clone());

    let file = parse_import_file(&path)?;

    let base_dir = path.parent().unwrap_or_else(|| Path::new(""));
    load_child_imports(&file, base_dir, units, visited, visiting, stack)?;

    stack.pop();
    visiting.remove(&path);
    visited.insert(path.clone());
    units.push(ParsedUnit { path, file });
    Ok(())
}

fn reject_import_cycle(
    path: &Path,
    visiting: &HashSet<PathBuf>,
    stack: &mut Vec<PathBuf>,
) -> Result<(), Error> {
    if !visiting.contains(path) {
        return Ok(());
    }

    stack.push(path.to_path_buf());
    let cycle = stack
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(" -> ");
    stack.pop();
    Err(Error::Import(format!("import cycle detected: {cycle}")))
}

fn parse_import_file(path: &Path) -> Result<SynFile, Error> {
    let source = fs::read_to_string(path)
        .map_err(|e| Error::Import(format!("error reading import `{}`: {e}", path.display())))?;
    synapse_parser::ast::parse(&source)
        .map_err(|e| Error::Import(format!("error parsing import `{}`:\n{e}", path.display())))
}

fn load_child_imports(
    file: &SynFile,
    base_dir: &Path,
    units: &mut Vec<ParsedUnit>,
    visited: &mut HashSet<PathBuf>,
    visiting: &mut HashSet<PathBuf>,
    stack: &mut Vec<PathBuf>,
) -> Result<(), Error> {
    for item in &file.items {
        if let Item::Import(import) = item {
            load_import_unit(
                &base_dir.join(&import.path),
                units,
                visited,
                visiting,
                stack,
            )?;
        }
    }
    Ok(())
}

fn canonicalize_import_path(path: &Path) -> Result<PathBuf, Error> {
    fs::canonicalize(path)
        .map_err(|e| Error::Import(format!("error reading import `{}`: {e}", path.display())))
}

fn validate_import_graph(graph: &ImportGraph) -> Result<(), Error> {
    let units_by_path = units_by_path(graph);
    for unit in &graph.units {
        validate_import_unit(unit, &units_by_path)?;
    }
    Ok(())
}

fn units_by_path(graph: &ImportGraph) -> HashMap<PathBuf, &ParsedUnit> {
    graph
        .units
        .iter()
        .map(|unit| (unit.path.clone(), unit))
        .collect()
}

fn imported_constants_for_unit(
    unit: &ParsedUnit,
    units_by_path: &HashMap<PathBuf, &ParsedUnit>,
) -> Result<synapse_codegen_cfs::ResolvedConstants, Error> {
    let base_dir = unit.path.parent().unwrap_or_else(|| Path::new(""));
    let mut constants = synapse_codegen_cfs::ResolvedConstants::new();

    for item in &unit.file.items {
        let Item::Import(import) = item else {
            continue;
        };

        let import_path = canonicalize_import_path(&base_dir.join(&import.path))?;
        let imported = units_by_path
            .get(&import_path)
            .expect("import graph loader parsed direct imports");
        constants.extend(exported_constants_for_unit(imported, units_by_path)?);
    }

    Ok(constants)
}

fn exported_constants_for_unit(
    unit: &ParsedUnit,
    units_by_path: &HashMap<PathBuf, &ParsedUnit>,
) -> Result<synapse_codegen_cfs::ResolvedConstants, Error> {
    let imported_constants = imported_constants_for_unit(unit, units_by_path)?;
    let resolved = synapse_codegen_cfs::resolve_integer_constants(&unit.file, &imported_constants);
    let local_namespace = namespace(&unit.file);
    let mut exported = synapse_codegen_cfs::ResolvedConstants::new();

    for item in &unit.file.items {
        let Item::Const(c) = item else {
            continue;
        };

        let key = if local_namespace.is_empty() {
            vec![c.name.clone()]
        } else {
            let mut qualified = local_namespace.clone();
            qualified.push(c.name.clone());
            qualified
        };
        if let Some(value) = resolved.get(&key) {
            exported.insert(key, *value);
        }
    }

    Ok(exported)
}

fn validate_import_unit(
    unit: &ParsedUnit,
    units_by_path: &HashMap<PathBuf, &ParsedUnit>,
) -> Result<(), Error> {
    let base_dir = unit.path.parent().unwrap_or_else(|| Path::new(""));
    let local_namespace = namespace(&unit.file);
    let mut symbols = local_symbols(&unit.file, &local_namespace);
    let mut imported_type_suggestions = HashMap::new();

    for item in &unit.file.items {
        let Item::Import(import) = item else {
            continue;
        };

        let import_path = canonicalize_import_path(&base_dir.join(&import.path))?;
        let imported = units_by_path
            .get(&import_path)
            .expect("import graph loader parsed direct imports");
        let imported_namespace = namespace(&imported.file);
        let imported_names = declared_names(&imported.file);
        for name in &imported_names {
            if !imported_namespace.is_empty() {
                let mut qualified = imported_namespace.clone();
                qualified.push(name.clone());
                imported_type_suggestions.insert(name.clone(), qualified.join("::"));
            }
        }
        symbols.extend(qualified_symbols(&imported_names, &imported_namespace));
    }

    validate_type_refs(&unit.file, &symbols, &imported_type_suggestions)
}

#[derive(Debug, Clone)]
struct MissionPacket {
    path: PathBuf,
    namespace: Vec<String>,
    name: String,
    kind: synapse_codegen_cfs::CfsPacketKind,
    mid: u64,
    cc: Option<u64>,
}

fn validate_mission_registry(
    graph: &ImportGraph,
    units_by_path: &HashMap<PathBuf, &ParsedUnit>,
) -> Result<(), Error> {
    let mut telemetry_mids = HashMap::<u64, MissionPacket>::new();
    let mut command_codes = HashMap::<(u64, u64), MissionPacket>::new();

    for unit in &graph.units {
        let imported_constants = imported_constants_for_unit(unit, units_by_path)?;
        let packets = synapse_codegen_cfs::collect_cfs_packets_with_constants(
            &unit.file,
            &imported_constants,
        )?;

        for packet in packets {
            let packet = mission_packet(unit, packet);
            register_mission_packet(&packet, &mut telemetry_mids, &mut command_codes)?;
        }
    }

    Ok(())
}

fn mission_packet(unit: &ParsedUnit, packet: synapse_codegen_cfs::CfsPacket) -> MissionPacket {
    MissionPacket {
        path: unit.path.clone(),
        namespace: packet.namespace,
        name: packet.name,
        kind: packet.kind,
        mid: packet.mid,
        cc: packet.cc,
    }
}

fn register_mission_packet(
    packet: &MissionPacket,
    telemetry_mids: &mut HashMap<u64, MissionPacket>,
    command_codes: &mut HashMap<(u64, u64), MissionPacket>,
) -> Result<(), Error> {
    match packet.kind {
        synapse_codegen_cfs::CfsPacketKind::Telemetry => {
            register_mission_telemetry(packet, telemetry_mids)
        }
        synapse_codegen_cfs::CfsPacketKind::Command => {
            register_mission_command(packet, command_codes)
        }
    }
}

fn register_mission_telemetry(
    packet: &MissionPacket,
    telemetry_mids: &mut HashMap<u64, MissionPacket>,
) -> Result<(), Error> {
    if let Some(first) = telemetry_mids.insert(packet.mid, packet.clone()) {
        return Err(Error::Mission(format!(
            "duplicate telemetry MID `{}` across mission packets `{}` ({}) and `{}` ({})",
            format_mid(packet.mid),
            packet_name(&first),
            first.path.display(),
            packet_name(packet),
            packet.path.display()
        )));
    }
    Ok(())
}

fn register_mission_command(
    packet: &MissionPacket,
    command_codes: &mut HashMap<(u64, u64), MissionPacket>,
) -> Result<(), Error> {
    let cc = packet
        .cc
        .expect("cFS packet collector resolves command codes");
    if let Some(first) = command_codes.insert((packet.mid, cc), packet.clone()) {
        return Err(Error::Mission(format!(
            "duplicate command MID/CC pair `{}`/`{}` across mission packets `{}` ({}) and `{}` ({})",
            format_mid(packet.mid),
            cc,
            packet_name(&first),
            first.path.display(),
            packet_name(packet),
            packet.path.display()
        )));
    }
    Ok(())
}

fn packet_name(packet: &MissionPacket) -> String {
    if packet.namespace.is_empty() {
        packet.name.clone()
    } else {
        let mut segments = packet.namespace.clone();
        segments.push(packet.name.clone());
        segments.join("::")
    }
}

pub(crate) fn format_mid(mid: u64) -> String {
    format!("0x{mid:04X}")
}

fn output_path_for(input: &Path, out_dir: &Path, lang: Lang) -> Result<PathBuf, Error> {
    let stem = input.file_stem().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("input file has no stem: {}", input.display()),
        )
    })?;
    Ok(out_dir.join(format!("{}.{}", stem.to_string_lossy(), lang.extension())))
}

fn namespace(file: &SynFile) -> Vec<String> {
    file.items
        .iter()
        .find_map(|item| match item {
            Item::Namespace(ns) => Some(ns.name.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

fn local_symbols(file: &SynFile, namespace: &[String]) -> HashSet<Vec<String>> {
    let mut symbols = HashSet::new();
    for name in declared_names(file) {
        symbols.insert(vec![name.clone()]);
        if !namespace.is_empty() {
            let mut qualified = namespace.to_vec();
            qualified.push(name);
            symbols.insert(qualified);
        }
    }
    symbols
}

fn qualified_symbols(names: &[String], namespace: &[String]) -> HashSet<Vec<String>> {
    let mut symbols = HashSet::new();
    if namespace.is_empty() {
        return symbols;
    }
    for name in names {
        let mut qualified = namespace.to_vec();
        qualified.push(name.clone());
        symbols.insert(qualified);
    }
    symbols
}

fn declared_names(file: &SynFile) -> Vec<String> {
    file.items
        .iter()
        .filter_map(|item| match item {
            Item::Const(c) => Some(c.name.clone()),
            Item::Enum(e) => Some(e.name.clone()),
            Item::Struct(s) | Item::Table(s) => Some(s.name.clone()),
            Item::Command(m) | Item::Telemetry(m) | Item::Message(m) => Some(m.name.clone()),
            Item::Namespace(_) | Item::Import(_) => None,
        })
        .collect()
}

fn validate_type_refs(
    file: &SynFile,
    symbols: &HashSet<Vec<String>>,
    imported_type_suggestions: &HashMap<String, String>,
) -> Result<(), Error> {
    for item in &file.items {
        validate_item_type_refs(item, symbols, imported_type_suggestions)?;
    }
    Ok(())
}

fn validate_item_type_refs(
    item: &Item,
    symbols: &HashSet<Vec<String>>,
    imported_type_suggestions: &HashMap<String, String>,
) -> Result<(), Error> {
    match item {
        Item::Const(c) => {
            validate_type_ref(&c.name, &c.ty.base, symbols, imported_type_suggestions)
        }
        Item::Struct(s) | Item::Table(s) => {
            validate_field_refs(&s.name, &s.fields, symbols, imported_type_suggestions)
        }
        Item::Command(m) | Item::Telemetry(m) | Item::Message(m) => {
            validate_field_refs(&m.name, &m.fields, symbols, imported_type_suggestions)
        }
        Item::Namespace(_) | Item::Import(_) | Item::Enum(_) => Ok(()),
    }
}

fn validate_field_refs(
    container: &str,
    fields: &[FieldDef],
    symbols: &HashSet<Vec<String>>,
    imported_type_suggestions: &HashMap<String, String>,
) -> Result<(), Error> {
    for field in fields {
        validate_type_ref(
            &format!("{container}.{}", field.name),
            &field.ty.base,
            symbols,
            imported_type_suggestions,
        )?;
    }
    Ok(())
}

fn validate_type_ref(
    owner: &str,
    base: &BaseType,
    symbols: &HashSet<Vec<String>>,
    imported_type_suggestions: &HashMap<String, String>,
) -> Result<(), Error> {
    let BaseType::Ref(segments) = base else {
        return Ok(());
    };
    if symbols.contains(segments) {
        return Ok(());
    }
    if segments.len() == 1 {
        if let Some(suggestion) = imported_type_suggestions.get(&segments[0]) {
            return Err(Error::Import(format!(
                "imported type reference `{}` in `{owner}` must be namespace-qualified as `{suggestion}`",
                segments[0]
            )));
        }
    }
    Err(Error::Import(format!(
        "unresolved type reference `{}` in `{owner}`",
        segments.join("::")
    )))
}

/// Generate a cFS C header from an input file.
pub fn generate_c_file(
    input: impl AsRef<Path>,
    out_dir: impl AsRef<Path>,
) -> Result<PathBuf, Error> {
    generate_file(input, out_dir, Lang::C)
}

/// Generate Rust `#[repr(C)]` bindings from an input file.
pub fn generate_rust_file(
    input: impl AsRef<Path>,
    out_dir: impl AsRef<Path>,
) -> Result<PathBuf, Error> {
    generate_file(input, out_dir, Lang::Rust)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn generate_c_from_string() {
        let out = generate_str(
            "@mid(0x1880)\n@cc(1)\ncommand SetMode { mode: u8 }",
            Lang::C,
        )
        .unwrap();
        assert!(out.contains("#define SET_MODE_MID  0x1880U"));
        assert!(out.contains("#define SET_MODE_CC   1U"));
        assert!(out.contains("CFE_MSG_CommandHeader_t Header;"));
    }

    #[test]
    fn generate_rust_from_string() {
        let out = generate_str("@mid(0x0801)\ntelemetry NavState { x: f64 }", Lang::Rust).unwrap();
        assert!(out.contains("pub const NAV_STATE_MID: u16 = 0x0801;"));
        assert!(out.contains("pub cfs_header: cfs_sys::CFE_MSG_TelemetryHeader_t,"));
    }

    #[test]
    fn rejects_optional_fields() {
        let err = generate_str(
            "@mid(0x0801)\ntelemetry Status { error_code?: u32 }",
            Lang::C,
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "optional field `Status.error_code` is not supported by cFS codegen yet"
        );
    }

    #[test]
    fn check_str_validates_cfs_codegen_support() {
        let err = check_str("@mid(0x0801)\ntelemetry Status { error_code?: u32 }").unwrap_err();
        assert_eq!(
            err.to_string(),
            "optional field `Status.error_code` is not supported by cFS codegen yet"
        );
    }

    #[test]
    fn rejects_default_values() {
        let err = generate_str("table Config { exposure_us: u32 = 10000 }", Lang::C).unwrap_err();
        assert_eq!(
            err.to_string(),
            "default value for field `Config.exposure_us` is not supported by cFS codegen yet"
        );
    }

    #[test]
    fn rejects_enum_fields() {
        let err = generate_str(
            "enum CameraMode { Idle = 0 Streaming = 1 }\n@mid(0x0801)\ntelemetry Status { mode: CameraMode }",
            Lang::C,
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "enum field `Status.mode` with type `CameraMode` needs an explicit integer representation for cFS codegen"
        );
    }

    #[test]
    fn rejects_unbounded_strings() {
        let err = generate_str("struct Label { name: string }", Lang::C).unwrap_err();
        assert_eq!(
            err.to_string(),
            "unbounded string field `Label.name` is not supported by cFS codegen; use `string[<=N]` or `string[N]`"
        );
    }

    #[test]
    fn rejects_legacy_message() {
        let err = generate_str("@mid(0x0801)\nmessage Status { x: f32 }", Lang::C).unwrap_err();
        assert_eq!(
            err.to_string(),
            "legacy message `Status` is not supported by cFS codegen; use `command` or `telemetry`"
        );
    }

    #[test]
    fn rejects_packet_without_mid() {
        let err = generate_str("command SetMode { mode: u8 }", Lang::C).unwrap_err();
        assert_eq!(
            err.to_string(),
            "packet `SetMode` is missing required `@mid(...)`"
        );
    }

    #[test]
    fn rejects_command_without_cc() {
        let err = generate_str("@mid(0x1880)\ncommand SetMode { mode: u8 }", Lang::C).unwrap_err();
        assert_eq!(
            err.to_string(),
            "command `SetMode` is missing required `@cc(...)`"
        );
    }

    #[test]
    fn rejects_mid_range_mismatch() {
        let err = generate_str(
            "@mid(0x0801)\n@cc(1)\ncommand SetMode { mode: u8 }",
            Lang::C,
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "packet `SetMode` has MID `0x0801U`, expected command MID with bit 0x1000 set"
        );
    }

    #[test]
    fn rejects_dynamic_arrays() {
        let err =
            generate_str("@mid(0x0801)\ntelemetry Samples { values: f32[] }", Lang::C).unwrap_err();
        assert_eq!(
            err.to_string(),
            "dynamic array field `Samples.values` with type `f32[]` is not supported by cFS codegen yet"
        );
    }

    #[test]
    fn rejects_non_string_bounded_arrays() {
        let err = generate_str("table Buffer { bytes: u8[<=256] }", Lang::C).unwrap_err();
        assert_eq!(
            err.to_string(),
            "bounded array field `Buffer.bytes` with type `u8[<=256]` is not supported by cFS codegen yet"
        );
    }

    #[test]
    fn generate_path_validates_imported_type_refs() {
        let dir = test_dir("validates-imported-type-refs");
        fs::write(
            dir.join("std_msgs.syn"),
            "namespace std_msgs\nstruct Header { seq: u32 }",
        )
        .unwrap();
        let input = dir.join("camera.syn");
        fs::write(
            &input,
            r#"namespace camera_app
import "std_msgs.syn"
@mid(0x0881)
telemetry CameraStatus {
    header: std_msgs::Header
}
"#,
        )
        .unwrap();

        let out = generate_path(&input, Lang::C).unwrap();
        assert!(out.contains("#include \"std_msgs.h\""));
        assert!(out.contains("std_msgs_Header_t header;"));
    }

    #[test]
    fn check_path_validates_imported_codegen_support() {
        let dir = test_dir("check-validates-imported-codegen-support");
        fs::write(
            dir.join("bad.syn"),
            "namespace bad\nstruct Unsupported { count?: u32 }",
        )
        .unwrap();
        let input = dir.join("root.syn");
        fs::write(
            &input,
            r#"namespace root
import "bad.syn"
struct Root { unsupported: bad::Unsupported }
"#,
        )
        .unwrap();

        let err = check_path(&input).unwrap_err();
        assert_eq!(
            err.to_string(),
            "optional field `Unsupported.count` is not supported by cFS codegen yet"
        );
    }

    #[test]
    fn check_paths_rejects_duplicate_telemetry_mids_across_roots() {
        let dir = test_dir("check-paths-duplicate-telemetry-mids");
        let nav = dir.join("nav.syn");
        fs::write(
            &nav,
            "namespace nav_app\n@mid(0x0801)\ntelemetry NavState { x: f64 }",
        )
        .unwrap();
        let payload = dir.join("payload.syn");
        fs::write(
            &payload,
            "namespace payload_app\n@mid(0x0801)\ntelemetry PayloadStatus { temp: f32 }",
        )
        .unwrap();

        let err = check_paths([&nav, &payload]).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("duplicate telemetry MID `0x0801`"));
        assert!(msg.contains("nav_app::NavState"));
        assert!(msg.contains("payload_app::PayloadStatus"));
    }

    #[test]
    fn check_paths_rejects_duplicate_command_mid_cc_pairs_across_roots() {
        let dir = test_dir("check-paths-duplicate-command-codes");
        let camera = dir.join("camera.syn");
        fs::write(
            &camera,
            "namespace camera_app\n@mid(0x1880)\n@cc(1)\ncommand SetMode { mode: u8 }",
        )
        .unwrap();
        let radio = dir.join("radio.syn");
        fs::write(
            &radio,
            "namespace radio_app\n@mid(0x1880)\n@cc(1)\ncommand SetMode { mode: u8 }",
        )
        .unwrap();

        let err = check_paths([&camera, &radio]).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("duplicate command MID/CC pair `0x1880`/`1`"));
        assert!(msg.contains("camera_app::SetMode"));
        assert!(msg.contains("radio_app::SetMode"));
    }

    #[test]
    fn check_paths_allows_shared_command_mid_with_distinct_command_codes() {
        let dir = test_dir("check-paths-shared-command-mid");
        let camera = dir.join("camera.syn");
        fs::write(
            &camera,
            "namespace camera_app\n@mid(0x1880)\n@cc(1)\ncommand SetMode { mode: u8 }",
        )
        .unwrap();
        let radio = dir.join("radio.syn");
        fs::write(
            &radio,
            "namespace radio_app\n@mid(0x1880)\n@cc(2)\ncommand SetMode { mode: u8 }",
        )
        .unwrap();

        check_paths([&camera, &radio]).unwrap();
    }

    #[test]
    fn generate_docs_includes_packet_ids_fields_and_docs() {
        let dir = test_dir("generate-docs");
        fs::write(
            dir.join("mission_ids.syn"),
            "namespace mission_ids\nconst NAV_STATE_MID: u16 = 0x0801",
        )
        .unwrap();
        let input = dir.join("nav.syn");
        fs::write(
            &input,
            r#"namespace nav_app
import "mission_ids.syn"
/// Navigation mode.
enum u8 NavMode {
    /// Tracking target attitude.
    Track = 1
}
/// Navigation state estimate.
@mid(mission_ids::NAV_STATE_MID)
telemetry NavState {
    /// Current mode.
    mode: NavMode
}
"#,
        )
        .unwrap();

        let html = generate_docs([&input]).unwrap();
        assert!(html.contains("Synapse Message Documentation"));
        assert!(html.contains("nav_app"));
        assert!(html.contains("NavState"));
        assert!(html.contains("0x0801"));
        assert!(html.contains("Current mode."));
        assert!(html.contains("NavMode"));
        assert!(html.contains("id=\"doc-search\""));
        assert!(html.contains("data-search="));
        assert!(html.contains("href=\"#"));
        assert!(!html.contains("file://"));
        assert!(!html.contains(&dir.display().to_string()));
        assert!(html.contains(">Source</a>"));
    }

    #[test]
    fn write_docs_writes_index_html() {
        let dir = test_dir("write-docs");
        let input = dir.join("status.syn");
        fs::write(
            &input,
            "namespace status_app\n@mid(0x0801)\ntelemetry Status { count: u32 }",
        )
        .unwrap();

        let out_dir = dir.join("site");
        let out_path = write_docs([&input], &out_dir).unwrap();
        assert_eq!(out_path, out_dir.join("index.html"));
        assert!(fs::read_to_string(out_path).unwrap().contains("status_app"));
    }

    #[test]
    fn generate_registry_json_includes_packet_facts() {
        let dir = test_dir("generate-registry-json");
        fs::write(
            dir.join("mission_ids.syn"),
            "namespace mission_ids\nconst CMD_MID: u16 = 0x1880\nconst SET_MODE_CC: u16 = 2",
        )
        .unwrap();
        let input = dir.join("camera.syn");
        fs::write(
            &input,
            r#"namespace camera_app
import "mission_ids.syn"
@mid(mission_ids::CMD_MID)
@cc(mission_ids::SET_MODE_CC)
command SetMode {
    mode: u8
}
"#,
        )
        .unwrap();

        let json = generate_registry([&input], RegistryFormat::Json).unwrap();
        assert!(json.contains("\"packets\""));
        assert!(json.contains("\"namespace\": \"camera_app\""));
        assert!(json.contains("\"qualified_name\": \"camera_app::SetMode\""));
        assert!(json.contains("\"kind\": \"command\""));
        assert!(json.contains("\"mid\": 6272"));
        assert!(json.contains("\"mid_hex\": \"0x1880\""));
        assert!(json.contains("\"cc\": 2"));
    }

    #[test]
    fn write_registry_csv_writes_packet_rows() {
        let dir = test_dir("write-registry-csv");
        let input = dir.join("status.syn");
        fs::write(
            &input,
            "namespace status_app\n@mid(0x0801)\ntelemetry Status { count: u32 }",
        )
        .unwrap();
        let output = dir.join("registry.csv");

        let out_path = write_registry([&input], &output, RegistryFormat::Csv).unwrap();
        assert_eq!(out_path, output);
        let csv = fs::read_to_string(out_path).unwrap();
        assert!(csv.starts_with("namespace,name,qualified_name,kind,source,mid,mid_hex,cc"));
        assert!(csv.contains("\"status_app\",\"Status\",\"status_app::Status\",\"telemetry\""));
        assert!(csv.contains("\"2049\",\"0x0801\""));
    }

    #[test]
    fn generate_path_resolves_imported_constants_in_attrs() {
        let dir = test_dir("resolves-imported-constants-in-attrs");
        fs::write(
            dir.join("nav_ids.syn"),
            "namespace nav_ids\nconst NAV_STATE_MID: u16 = 0x0801",
        )
        .unwrap();
        let input = dir.join("nav.syn");
        fs::write(
            &input,
            r#"namespace nav_app
import "nav_ids.syn"
@mid(nav_ids::NAV_STATE_MID)
telemetry NavState {
    x: f64
}
"#,
        )
        .unwrap();

        let out = generate_path(&input, Lang::C).unwrap();
        assert!(out.contains("#define NAV_STATE_MID  0x0801U"));
    }

    #[test]
    fn generate_path_resolves_imported_alias_constants_in_attrs() {
        let dir = test_dir("resolves-imported-alias-constants-in-attrs");
        fs::write(
            dir.join("mission_ids.syn"),
            "namespace mission_ids\nconst NAV_CMD_MID: u16 = 0x1880",
        )
        .unwrap();
        fs::write(
            dir.join("nav_ids.syn"),
            r#"namespace nav_ids
import "mission_ids.syn"
const SET_MODE_MID: u16 = mission_ids::NAV_CMD_MID
const SET_MODE_CC: u16 = 2
"#,
        )
        .unwrap();
        let input = dir.join("nav.syn");
        fs::write(
            &input,
            r#"namespace nav_app
import "nav_ids.syn"
@mid(nav_ids::SET_MODE_MID)
@cc(nav_ids::SET_MODE_CC)
command SetMode {
    mode: u8
}
"#,
        )
        .unwrap();

        let out = generate_path(&input, Lang::Rust).unwrap();
        assert!(out.contains("pub const SET_MODE_MID: u16 = 0x1880;"));
        assert!(out.contains("pub const SET_MODE_CC: u16 = 2;"));
    }

    #[test]
    fn generate_path_rejects_transitive_only_constant_refs() {
        let dir = test_dir("rejects-transitive-only-constant-refs");
        fs::write(
            dir.join("mission_ids.syn"),
            "namespace mission_ids\nconst NAV_STATE_MID: u16 = 0x0801",
        )
        .unwrap();
        fs::write(
            dir.join("nav_ids.syn"),
            r#"namespace nav_ids
import "mission_ids.syn"
const LOCAL_MID: u16 = mission_ids::NAV_STATE_MID
"#,
        )
        .unwrap();
        let input = dir.join("nav.syn");
        fs::write(
            &input,
            r#"namespace nav_app
import "nav_ids.syn"
@mid(mission_ids::NAV_STATE_MID)
telemetry NavState {
    x: f64
}
"#,
        )
        .unwrap();

        let err = generate_path(&input, Lang::C).unwrap_err();
        assert_eq!(
            err.to_string(),
            "packet `NavState` has unresolved or non-integer `@mid(...)`; cFS codegen requires an integer, hex, local integer constant, or imported integer constant message ID"
        );
    }

    #[test]
    fn generate_path_rejects_unqualified_imported_type_refs() {
        let dir = test_dir("rejects-unqualified-imported-type-refs");
        fs::write(
            dir.join("std_msgs.syn"),
            "namespace std_msgs\nstruct Header { seq: u32 }",
        )
        .unwrap();
        let input = dir.join("camera.syn");
        fs::write(
            &input,
            r#"namespace camera_app
import "std_msgs.syn"
@mid(0x0881)
telemetry CameraStatus {
    header: Header
}
"#,
        )
        .unwrap();

        let err = generate_path(&input, Lang::C).unwrap_err();
        assert_eq!(
            err.to_string(),
            "imported type reference `Header` in `CameraStatus.header` must be namespace-qualified as `std_msgs::Header`"
        );
    }

    #[test]
    fn generate_path_rejects_missing_import_file() {
        let dir = test_dir("missing-import");
        let input = dir.join("camera.syn");
        fs::write(&input, r#"import "missing.syn""#).unwrap();

        let err = generate_path(&input, Lang::C).unwrap_err();
        assert!(err.to_string().contains("error reading import"));
        assert!(err.to_string().contains("missing.syn"));
    }

    #[test]
    fn generate_path_rejects_unresolved_type_ref() {
        let dir = test_dir("unresolved-type-ref");
        let input = dir.join("camera.syn");
        fs::write(
            &input,
            "@mid(0x0881)\ntelemetry CameraStatus { header: std_msgs::Header }",
        )
        .unwrap();

        let err = generate_path(&input, Lang::C).unwrap_err();
        assert_eq!(
            err.to_string(),
            "unresolved type reference `std_msgs::Header` in `CameraStatus.header`"
        );
    }

    #[test]
    fn generate_path_validates_transitive_imports() {
        let dir = test_dir("validates-transitive-imports");
        fs::write(
            dir.join("time.syn"),
            "namespace time\nstruct Time { sec: u32 }",
        )
        .unwrap();
        fs::write(
            dir.join("std_msgs.syn"),
            r#"namespace std_msgs
import "time.syn"
struct Header { stamp: time::Time }
"#,
        )
        .unwrap();
        let input = dir.join("camera.syn");
        fs::write(
            &input,
            r#"namespace camera_app
import "std_msgs.syn"
@mid(0x0881)
telemetry CameraStatus {
    header: std_msgs::Header
}
"#,
        )
        .unwrap();

        let out = generate_path(&input, Lang::C).unwrap();
        assert!(out.contains("#include \"std_msgs.h\""));
        assert!(out.contains("std_msgs_Header_t header;"));
    }

    #[test]
    fn generate_path_rejects_missing_transitive_import_file() {
        let dir = test_dir("missing-transitive-import");
        fs::write(
            dir.join("std_msgs.syn"),
            r#"namespace std_msgs
import "missing_time.syn"
struct Header { seq: u32 }
"#,
        )
        .unwrap();
        let input = dir.join("camera.syn");
        fs::write(
            &input,
            r#"namespace camera_app
import "std_msgs.syn"
@mid(0x0881)
telemetry CameraStatus {
    header: std_msgs::Header
}
"#,
        )
        .unwrap();

        let err = generate_path(&input, Lang::C).unwrap_err();
        assert!(err.to_string().contains("error reading import"));
        assert!(err.to_string().contains("missing_time.syn"));
    }

    #[test]
    fn generate_path_rejects_references_to_transitive_only_imports() {
        let dir = test_dir("rejects-transitive-only-import-reference");
        fs::write(
            dir.join("time.syn"),
            "namespace time\nstruct Time { sec: u32 }",
        )
        .unwrap();
        fs::write(
            dir.join("std_msgs.syn"),
            r#"namespace std_msgs
import "time.syn"
struct Header { stamp: time::Time }
"#,
        )
        .unwrap();
        let input = dir.join("camera.syn");
        fs::write(
            &input,
            r#"namespace camera_app
import "std_msgs.syn"
@mid(0x0881)
telemetry CameraStatus {
    stamp: time::Time
}
"#,
        )
        .unwrap();

        let err = generate_path(&input, Lang::C).unwrap_err();
        assert_eq!(
            err.to_string(),
            "unresolved type reference `time::Time` in `CameraStatus.stamp`"
        );
    }

    #[test]
    fn generate_files_writes_import_closure_in_dependency_order() {
        let dir = test_dir("generate-files-import-closure");
        fs::write(
            dir.join("time.syn"),
            "namespace time\nstruct Time { sec: u32 }",
        )
        .unwrap();
        fs::write(
            dir.join("std_msgs.syn"),
            r#"namespace std_msgs
import "time.syn"
struct Header { stamp: time::Time }
"#,
        )
        .unwrap();
        let input = dir.join("camera.syn");
        fs::write(
            &input,
            r#"namespace camera_app
import "std_msgs.syn"
@mid(0x0881)
telemetry CameraStatus {
    header: std_msgs::Header
}
"#,
        )
        .unwrap();

        let out_dir = dir.join("generated");
        let written = generate_files(&input, &out_dir, Lang::C).unwrap();
        let names = written
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect::<Vec<_>>();
        assert_eq!(names, ["time.h", "std_msgs.h", "camera.h"]);
        assert!(
            fs::read_to_string(out_dir.join("std_msgs.h"))
                .unwrap()
                .contains("#include \"time.h\"")
        );
        assert!(
            fs::read_to_string(out_dir.join("camera.h"))
                .unwrap()
                .contains("#include \"std_msgs.h\"")
        );
    }

    #[test]
    fn generate_path_rejects_import_cycles() {
        let dir = test_dir("rejects-import-cycles");
        fs::write(
            dir.join("a.syn"),
            r#"namespace a
import "b.syn"
struct A { b: b::B }
"#,
        )
        .unwrap();
        fs::write(
            dir.join("b.syn"),
            r#"namespace b
import "a.syn"
struct B { a: a::A }
"#,
        )
        .unwrap();

        let err = generate_path(dir.join("a.syn"), Lang::C).unwrap_err();
        assert!(err.to_string().contains("import cycle detected"));
        assert!(err.to_string().contains("a.syn"));
        assert!(err.to_string().contains("b.syn"));
    }

    fn test_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("synapse-{name}-{}-{stamp}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
