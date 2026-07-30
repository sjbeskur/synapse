mod c;
mod constants;
mod error;
mod rust;
mod types;
mod util;
mod validate;

use synapse_parser::ast::SynFile;

pub use c::{generate_c, try_generate_c, try_generate_c_with_constants};
use constants::const_context;
pub use error::CodegenError;
pub use rust::{generate_rust, try_generate_rust, try_generate_rust_with_constants};
pub use types::{
    CfsPacket, CfsPacketKind, GENERATED_BANNER, PREAMBLE, ResolvedConstants, RustOptions,
};
pub use validate::{collect_cfs_packets_with_constants, validate_cfs, validate_cfs_with_constants};

/// Resolve this file's integer constants, including aliases to visible imported constants.
pub fn resolve_integer_constants(
    file: &SynFile,
    imported_constants: &ResolvedConstants,
) -> ResolvedConstants {
    let constants = const_context(file, imported_constants);
    constants.resolved_local_constants()
}

#[cfg(test)]
mod tests;
