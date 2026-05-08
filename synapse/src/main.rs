use std::{path::PathBuf, process};

use clap::{Args as ClapArgs, CommandFactory, Parser, Subcommand, ValueEnum, error::ErrorKind};

#[derive(Parser)]
#[command(
    name = "synapse",
    about = "NASA cFS message definition compiler — generates C headers and Rust bindings from .syn files"
)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,

    #[command(flatten)]
    generate: GenerateArgs,
}

#[derive(Subcommand)]
enum Command {
    /// Validate a .syn file and its imports without writing generated output.
    Check(CheckArgs),
    /// Generate C headers or Rust bindings.
    Generate(GenerateArgs),
}

#[derive(ClapArgs)]
struct CheckArgs {
    /// Input .syn file
    file: PathBuf,
}

#[derive(ClapArgs)]
struct GenerateArgs {
    /// Target language
    #[arg(long, value_enum)]
    lang: Option<CliLang>,

    /// Write output to this directory instead of stdout.
    /// By default, this writes the root file plus all transitive imports.
    #[arg(long, short = 'o')]
    out_dir: Option<PathBuf>,

    /// With --out-dir, generate only the requested root file.
    #[arg(long)]
    single_file: bool,

    /// Input .syn file
    file: Option<PathBuf>,
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
    match args.command {
        Some(Command::Check(check)) => check_path(check),
        Some(Command::Generate(generate)) => generate_path(generate),
        None => generate_path(args.generate),
    }
}

fn check_path(args: CheckArgs) {
    cfs_synapse::check_path(&args.file).unwrap_or_else(|e| {
        eprintln!("Error checking {}:\n{e}", args.file.display());
        process::exit(1);
    });
    eprintln!("checked {}", args.file.display());
}

fn generate_path(args: GenerateArgs) {
    let file = args.file.unwrap_or_else(|| {
        Args::command()
            .error(
                ErrorKind::MissingRequiredArgument,
                "missing required input .syn file",
            )
            .exit()
    });
    let lang = args.lang.map(cfs_synapse::Lang::from).unwrap_or_else(|| {
        Args::command()
            .error(
                ErrorKind::MissingRequiredArgument,
                "missing required `--lang <c|rust>`",
            )
            .exit()
    });

    match args.out_dir {
        None if args.single_file => {
            eprintln!(
                "Error generating {}: --single-file only applies with --out-dir",
                file.display()
            );
            process::exit(1);
        }
        None => {
            let output = cfs_synapse::generate_path(&file, lang).unwrap_or_else(|e| {
                eprintln!("Error generating {}:\n{e}", file.display());
                process::exit(1);
            });

            print!("{output}");
        }
        Some(dir) if args.single_file => {
            let out_path = cfs_synapse::generate_file(&file, &dir, lang).unwrap_or_else(|e| {
                eprintln!("Error generating {}: {e}", file.display());
                process::exit(1);
            });
            eprintln!("wrote {}", out_path.display());
        }
        Some(dir) => {
            let out_paths = cfs_synapse::generate_files(&file, &dir, lang).unwrap_or_else(|e| {
                eprintln!("Error generating {}: {e}", file.display());
                process::exit(1);
            });
            for out_path in out_paths {
                eprintln!("wrote {}", out_path.display());
            }
        }
    }
}
