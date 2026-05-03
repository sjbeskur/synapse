use std::{
    error::Error as StdError,
    fmt, fs,
    path::{Path, PathBuf},
};

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
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{e}"),
            Error::Parse(e) => write!(f, "{e}"),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            Error::Parse(e) => Some(e),
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

/// Generate code from `.syn` source text.
pub fn generate_str(source: &str, lang: Lang) -> Result<String, Error> {
    let file = synapse_parser::ast::parse(source)?;
    let output = match lang {
        Lang::C => synapse_codegen_cfs::generate_c(&file),
        Lang::Rust => synapse_codegen_cfs::generate_rust(&file, &Default::default()),
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
    let source = fs::read_to_string(input)?;
    let output = generate_str(&source, lang)?;

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

    #[test]
    fn generate_c_from_string() {
        let out = generate_str("@mid(0x1880)\ncommand SetMode { mode: u8 }", Lang::C).unwrap();
        assert!(out.contains("#define SET_MODE_MID  0x1880U"));
        assert!(out.contains("CFE_MSG_CommandHeader_t Header;"));
    }

    #[test]
    fn generate_rust_from_string() {
        let out = generate_str("@mid(0x0801)\ntelemetry NavState { x: f64 }", Lang::Rust).unwrap();
        assert!(out.contains("pub const NAV_STATE_MID: u16 = 0x0801;"));
        assert!(out.contains("pub cfs_header: cfs_sys::CFE_MSG_TelemetryHeader_t,"));
    }
}
