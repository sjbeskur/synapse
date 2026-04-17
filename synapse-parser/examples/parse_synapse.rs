use std::{env, fs, process};

use pest::Parser;
use synapse_parser::synapse::{SynapseParser, Rule};

fn main() {
    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: parse_synapse <file.syn>");
        process::exit(1);
    });

    let source = fs::read_to_string(&path).unwrap_or_else(|e| {
        eprintln!("Error reading {path}: {e}");
        process::exit(1);
    });

    match SynapseParser::parse(Rule::file, &source) {
        Ok(pairs) => {
            println!("✓ Parsed successfully: {path}");
            print_pairs(pairs, 0);
        }
        Err(e) => {
            eprintln!("✗ Parse error in {path}:\n{e}");
            process::exit(1);
        }
    }
}

fn print_pairs(pairs: pest::iterators::Pairs<Rule>, depth: usize) {
    for pair in pairs {
        let indent = "  ".repeat(depth);
        let span = pair.as_span();
        let text = pair.as_str();

        let preview = if text.len() > 40 {
            format!("{}…", &text[..40].replace('\n', "↵"))
        } else {
            text.replace('\n', "↵")
        };

        println!(
            "{indent}{rule:?}  [{start}..{end}]  {preview:?}",
            rule  = pair.as_rule(),
            start = span.start(),
            end   = span.end(),
        );

        print_pairs(pair.into_inner(), depth + 1);
    }
}
