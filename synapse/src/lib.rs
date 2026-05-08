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

/// Generate code from a `.syn` input path, validating direct imports relative to that file.
pub fn generate_path(input: impl AsRef<Path>, lang: Lang) -> Result<String, Error> {
    let input = input.as_ref();
    let source = fs::read_to_string(input)?;
    let file = synapse_parser::ast::parse(&source)?;
    validate_imports(input, &file)?;
    generate_parsed(&file, lang)
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

fn validate_imports(input: &Path, file: &SynFile) -> Result<(), Error> {
    let base_dir = input.parent().unwrap_or_else(|| Path::new(""));
    let local_namespace = namespace(file);
    let mut symbols = local_symbols(file, &local_namespace);
    let mut imported_type_suggestions = HashMap::new();

    for item in &file.items {
        let Item::Import(import) = item else {
            continue;
        };

        let import_path = base_dir.join(&import.path);
        let source = fs::read_to_string(&import_path).map_err(|e| {
            Error::Import(format!(
                "error reading import `{}`: {e}",
                import_path.display()
            ))
        })?;
        let imported = synapse_parser::ast::parse(&source).map_err(|e| {
            Error::Import(format!(
                "error parsing import `{}`:\n{e}",
                import_path.display()
            ))
        })?;
        let imported_namespace = namespace(&imported);
        let imported_names = declared_names(&imported);
        for name in &imported_names {
            if !imported_namespace.is_empty() {
                let mut qualified = imported_namespace.clone();
                qualified.push(name.clone());
                imported_type_suggestions.insert(name.clone(), qualified.join("::"));
            }
        }
        symbols.extend(qualified_symbols(&imported_names, &imported_namespace));
    }

    validate_type_refs(file, &symbols, &imported_type_suggestions)
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
