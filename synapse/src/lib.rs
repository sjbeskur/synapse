use std::{
    collections::{HashMap, HashSet},
    error::Error as StdError,
    fmt, fs,
    path::{Path, PathBuf},
};

use synapse_parser::ast::{BaseType, FieldDef, Item, SynFile};

/// Target language for Synapse code generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// NASA cFS C header (`.h`).
    C,
    /// Rust `#[repr(C)]` bindings (`.rs`).
    Rust,
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

/// Error type returned by the Synapse library facade.
#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Parse(Box<pest::error::Error<synapse_parser::synapse::Rule>>),
    Codegen(synapse_codegen_cfs::CodegenError),
    Import(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{e}"),
            Error::Parse(e) => write!(f, "{e}"),
            Error::Codegen(e) => write!(f, "{e}"),
            Error::Import(e) => write!(f, "{e}"),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            Error::Parse(e) => Some(e),
            Error::Codegen(e) => Some(e),
            Error::Import(_) => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Error::Io(value)
    }
}

impl From<pest::error::Error<synapse_parser::synapse::Rule>> for Error {
    fn from(value: pest::error::Error<synapse_parser::synapse::Rule>) -> Self {
        Error::Parse(Box::new(value))
    }
}

impl From<synapse_codegen_cfs::CodegenError> for Error {
    fn from(value: synapse_codegen_cfs::CodegenError) -> Self {
        Error::Codegen(value)
    }
}

/// Generate code from `.syn` source text.
pub fn generate_str(source: &str, lang: Lang) -> Result<String, Error> {
    let file = synapse_parser::ast::parse(source)?;
    generate_parsed(&file, lang)
}

/// Generate code from a `.syn` input path, validating the import graph rooted at that file.
pub fn generate_path(input: impl AsRef<Path>, lang: Lang) -> Result<String, Error> {
    let graph = load_import_graph(input.as_ref())?;
    validate_import_graph(&graph)?;
    let root = graph
        .units
        .last()
        .expect("import graph always contains the root input");
    generate_parsed(&root.file, lang)
}

fn generate_parsed(file: &SynFile, lang: Lang) -> Result<String, Error> {
    let output = match lang {
        Lang::C => synapse_codegen_cfs::try_generate_c(file)?,
        Lang::Rust => synapse_codegen_cfs::try_generate_rust(file, &Default::default())?,
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

    let out_dir = out_dir.as_ref();
    fs::create_dir_all(out_dir)?;

    let mut written = Vec::new();
    for unit in &graph.units {
        let output = generate_parsed(&unit.file, lang)?;
        let out_path = output_path_for(&unit.path, out_dir, lang)?;
        fs::write(&out_path, output)?;
        written.push(out_path);
    }
    Ok(written)
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
    let mut units = Vec::new();
    let mut visited = HashSet::new();
    let mut visiting = HashSet::new();
    let mut stack = Vec::new();
    load_import_unit(input, &mut units, &mut visited, &mut visiting, &mut stack)?;
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
    if visiting.contains(&path) {
        stack.push(path.clone());
        let cycle = stack
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(" -> ");
        stack.pop();
        return Err(Error::Import(format!("import cycle detected: {cycle}")));
    }

    visiting.insert(path.clone());
    stack.push(path.clone());

    let source = fs::read_to_string(&path)
        .map_err(|e| Error::Import(format!("error reading import `{}`: {e}", path.display())))?;
    let file = synapse_parser::ast::parse(&source)
        .map_err(|e| Error::Import(format!("error parsing import `{}`:\n{e}", path.display())))?;

    let base_dir = path.parent().unwrap_or_else(|| Path::new(""));
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

    stack.pop();
    visiting.remove(&path);
    visited.insert(path.clone());
    units.push(ParsedUnit { path, file });
    Ok(())
}

fn canonicalize_import_path(path: &Path) -> Result<PathBuf, Error> {
    fs::canonicalize(path)
        .map_err(|e| Error::Import(format!("error reading import `{}`: {e}", path.display())))
}

fn validate_import_graph(graph: &ImportGraph) -> Result<(), Error> {
    let units_by_path = graph
        .units
        .iter()
        .map(|unit| (unit.path.clone(), unit))
        .collect::<HashMap<_, _>>();
    for unit in &graph.units {
        validate_import_unit(unit, &units_by_path)?;
    }
    Ok(())
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
        match item {
            Item::Const(c) => {
                validate_type_ref(&c.name, &c.ty.base, symbols, imported_type_suggestions)?
            }
            Item::Struct(s) | Item::Table(s) => {
                validate_field_refs(&s.name, &s.fields, symbols, imported_type_suggestions)?
            }
            Item::Command(m) | Item::Telemetry(m) | Item::Message(m) => {
                validate_field_refs(&m.name, &m.fields, symbols, imported_type_suggestions)?
            }
            Item::Namespace(_) | Item::Import(_) | Item::Enum(_) => {}
        }
    }
    Ok(())
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
