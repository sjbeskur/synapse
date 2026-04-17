use std::{env, fs, process};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 4 || args[1] != "--lang" {
        eprintln!("Usage: synapse --lang <rust|cpp> <file.syn>");
        process::exit(1);
    }

    let lang = &args[2];
    let path = &args[3];

    let source = fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("Error reading {path}: {e}");
        process::exit(1);
    });

    let file = synapse_parser::ast::parse(&source).unwrap_or_else(|e| {
        eprintln!("Parse error in {path}:\n{e}");
        process::exit(1);
    });

    let output = match lang.as_str() {
        "rust" => synapse_codegen_rust::generate(&file),
        "cpp"  => synapse_codegen_cpp::generate(&file),
        other  => {
            eprintln!("Unknown language: {other}. Supported: rust, cpp");
            process::exit(1);
        }
    };

    print!("{}", output);
}
