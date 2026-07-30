use std::{error::Error as StdError, fmt};

/// Error type returned by the Synapse library facade.
#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Parse(Box<pest::error::Error<synapse_parser::synapse::Rule>>),
    Codegen(synapse_codegen_cfs::CodegenError),
    Import(String),
    Manifest(String),
    Mission(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(_) | Error::Parse(_) | Error::Codegen(_) => fmt_source_error(self, f),
            Error::Import(_) | Error::Manifest(_) | Error::Mission(_) => fmt_message_error(self, f),
        }
    }
}

fn fmt_source_error(error: &Error, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match error {
        Error::Io(e) => write!(f, "{e}"),
        Error::Parse(e) => write!(f, "{e}"),
        Error::Codegen(e) => write!(f, "{e}"),
        Error::Import(_) | Error::Manifest(_) | Error::Mission(_) => {
            unreachable!("non-source error")
        }
    }
}

fn fmt_message_error(error: &Error, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match error {
        Error::Import(e) | Error::Manifest(e) | Error::Mission(e) => write!(f, "{e}"),
        Error::Io(_) | Error::Parse(_) | Error::Codegen(_) => unreachable!("non-message error"),
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            Error::Parse(e) => Some(e),
            Error::Codegen(e) => Some(e),
            Error::Import(_) | Error::Manifest(_) | Error::Mission(_) => None,
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
