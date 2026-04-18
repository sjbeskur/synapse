use std::{env, fs, process};

fn main() {
    let args: Vec<String> = env::args().collect();

    // Accept: synapse --lang rust [--no-std] <file.syn>
    if args.len() < 4 || args[1] != "--lang" {
        eprintln!("Usage: synapse --lang <rust|cpp|cfs|cfs-rust> [--no-std] <file.syn>");
        process::exit(1);
    }

    let lang    = &args[2];
    let no_std  = args.iter().any(|a| a == "--no-std");
    let path    = args.last().unwrap();

    let source = fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("Error reading {path}: {e}");
        process::exit(1);
    });

    let file = synapse_parser::ast::parse(&source).unwrap_or_else(|e| {
        eprintln!("Parse error in {path}:\n{e}");
        process::exit(1);
    });

    let output = match lang.as_str() {
        "rust" if no_std => synapse_codegen_rust::generate_nostd(&file),
        "rust"           => synapse_codegen_rust::generate(&file),
        "cpp"            => synapse_codegen_cpp::generate(&file),
        "cfs"            => synapse_codegen_cfs::generate(&file),
        "cfs-rust"       => synapse_codegen_cfs::generate_rust(&file, &Default::default()),
        other => {
            eprintln!("Unknown language: {other}. Supported: rust, cpp, cfs, cfs-rust");
            process::exit(1);
        }
    };

    print!("{}", output);
}
