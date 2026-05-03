use std::{path::PathBuf, process};

use clap::{Parser, ValueEnum};

#[derive(Parser)]
#[command(
    name = "synapse",
    about = "NASA cFS message definition compiler — generates C headers and Rust bindings from .syn files"
)]
struct Args {
    /// Target language
    #[arg(long, value_enum)]
    lang: CliLang,

    /// Write output to this directory instead of stdout.
    /// Output file is named after the input file with the appropriate extension.
    #[arg(long, short = 'o')]
    out_dir: Option<PathBuf>,

    /// Input .syn file
    file: PathBuf,
}

#[derive(Clone, ValueEnum)]
enum CliLang {
    /// NASA cFS C header (.h)
    C,
    /// Rust #[repr(C)] bindings (.rs)
    Rust,
}

impl From<CliLang> for cfs_synapse::Lang {
    fn from(value: CliLang) -> Self {
        match value {
            CliLang::C => cfs_synapse::Lang::C,
            CliLang::Rust => cfs_synapse::Lang::Rust,
        }
    }
}

fn main() {
    let args = Args::parse();
    let lang = cfs_synapse::Lang::from(args.lang);

    match args.out_dir {
        None => {
            let source = std::fs::read_to_string(&args.file).unwrap_or_else(|e| {
                eprintln!("Error reading {}: {e}", args.file.display());
                process::exit(1);
            });

            let output = cfs_synapse::generate_str(&source, lang).unwrap_or_else(|e| {
                eprintln!("Error generating {}:\n{e}", args.file.display());
                process::exit(1);
            });

            print!("{output}");
        }
        Some(dir) => {
            let out_path = cfs_synapse::generate_file(&args.file, &dir, lang).unwrap_or_else(|e| {
                eprintln!("Error generating {}: {e}", args.file.display());
                process::exit(1);
            });
            eprintln!("wrote {}", out_path.display());
        }
    }
}
