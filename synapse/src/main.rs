use std::{fs, path::PathBuf, process};

use clap::{Parser, ValueEnum};

#[derive(Parser)]
#[command(
    name = "synapse",
    about = "NASA cFS message definition compiler — generates C headers and Rust bindings from .syn files"
)]
struct Args {
    /// Target language
    #[arg(long, value_enum)]
    lang: Lang,

    /// Write output to this directory instead of stdout.
    /// Output file is named after the input file with the appropriate extension.
    #[arg(long, short = 'o')]
    out_dir: Option<PathBuf>,

    /// Input .syn file
    file: PathBuf,
}

#[derive(Clone, ValueEnum)]
enum Lang {
    /// NASA cFS C header (.h)
    C,
    /// Rust #[repr(C)] bindings (.rs)
    Rust,
}

fn main() {
    let args = Args::parse();

    let path = &args.file;
    let source = fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("Error reading {}: {e}", path.display());
        process::exit(1);
    });

    let file = synapse_parser::ast::parse(&source).unwrap_or_else(|e| {
        eprintln!("Parse error in {}:\n{e}", path.display());
        process::exit(1);
    });

    let (output, ext) = match args.lang {
        Lang::C    => (synapse_codegen_cfs::generate_c(&file),    "h"),
        Lang::Rust => (synapse_codegen_cfs::generate_rust(&file, &Default::default()), "rs"),
    };

    match args.out_dir {
        None => print!("{output}"),
        Some(dir) => {
            fs::create_dir_all(&dir).unwrap_or_else(|e| {
                eprintln!("Error creating output directory {}: {e}", dir.display());
                process::exit(1);
            });

            let stem = path.file_stem()
                .expect("input file has no stem")
                .to_string_lossy();
            let out_path = dir.join(format!("{stem}.{ext}"));

            fs::write(&out_path, &output).unwrap_or_else(|e| {
                eprintln!("Error writing {}: {e}", out_path.display());
                process::exit(1);
            });

            eprintln!("wrote {}", out_path.display());
        }
    }
}
