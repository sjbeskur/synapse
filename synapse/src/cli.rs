use std::{path::PathBuf, process};

use clap::{Args as ClapArgs, CommandFactory, Parser, Subcommand, ValueEnum, error::ErrorKind};

#[derive(Parser)]
#[command(
    name = "synapse",
    about = "NASA cFS-friendly message contract utility",
    long_about = "Synapse validates .syn message contracts and generates cFS-oriented artifacts from the same source of truth: C headers, Rust repr(C) bindings, searchable HTML documentation, and packet registries.",
    after_long_help = "Examples:
  synapse --lang c -o generated synapse-integration-tests/syn/camera_msgs.syn
  synapse generate --lang rust -o generated synapse-integration-tests/syn/camera_msgs.syn
  synapse check examples/mission-demo/syn/nav_app.syn examples/mission-demo/syn/camera_app.syn
  synapse doc -o docs synapse-integration-tests/syn/camera_msgs.syn
  synapse registry --format csv -o registry.csv examples/mission-demo/syn/*.syn"
)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,

    #[command(flatten)]
    generate: GenerateArgs,
}

#[derive(Subcommand)]
enum Command {
    /// Validate .syn files and imports without writing generated output.
    #[command(
        long_about = "Validate one or more root .syn files, including their import closures. When multiple roots are provided, Synapse also checks mission-wide packet ID conflicts such as duplicate telemetry MIDs and duplicate command MID/CC pairs."
    )]
    Check(CheckArgs),
    /// Generate searchable static HTML documentation.
    #[command(
        long_about = "Generate a self-contained static HTML documentation site for one or more root .syn files. The generated page includes packet IDs, command codes, fields, doc comments, source links, and a sidebar search index."
    )]
    Doc(DocArgs),
    /// Generate C headers or Rust repr(C) bindings.
    #[command(
        long_about = "Generate ABI-oriented C headers or Rust repr(C) bindings from a root .syn file. When an output directory is provided, Synapse writes the root file and its transitive imports by default."
    )]
    Generate(GenerateArgs),
    /// Emit a machine-readable packet registry.
    #[command(
        long_about = "Emit a packet registry for one or more root .syn files. Registry output captures resolved packet facts such as namespace, packet name, kind, source file, MID, and command code."
    )]
    Registry(RegistryArgs),
}

#[derive(ClapArgs)]
#[command(after_long_help = "Examples:
  synapse check synapse-integration-tests/syn/camera_msgs.syn
  synapse check examples/mission-demo/syn/nav_app.syn examples/mission-demo/syn/camera_app.syn examples/mission-demo/syn/payload_app.syn")]
struct CheckArgs {
    /// Root .syn files to validate together.
    #[arg(required = true)]
    files: Vec<PathBuf>,
}

#[derive(ClapArgs)]
#[command(after_long_help = "Examples:
  synapse doc synapse-integration-tests/syn/camera_msgs.syn
  synapse doc -o docs synapse-integration-tests/syn/camera_msgs.syn
  synapse doc -o docs examples/mission-demo/syn/nav_app.syn examples/mission-demo/syn/camera_app.syn")]
struct DocArgs {
    /// Write index.html to this directory instead of stdout.
    #[arg(long, short = 'o')]
    out_dir: Option<PathBuf>,

    /// Root .syn files to document together.
    #[arg(required = true)]
    files: Vec<PathBuf>,
}

#[derive(ClapArgs)]
#[command(after_long_help = "Examples:
  synapse registry synapse-integration-tests/syn/camera_msgs.syn
  synapse registry --format csv -o registry.csv examples/mission-demo/syn/nav_app.syn examples/mission-demo/syn/camera_app.syn")]
struct RegistryArgs {
    /// Registry output format.
    #[arg(long, value_enum, default_value_t = CliRegistryFormat::Json)]
    format: CliRegistryFormat,

    /// Write registry output to this file instead of stdout.
    #[arg(long, short = 'o')]
    output: Option<PathBuf>,

    /// Root .syn files to export together.
    #[arg(required = true)]
    files: Vec<PathBuf>,
}

#[derive(ClapArgs)]
#[command(after_long_help = "Examples:
  synapse --lang c synapse-integration-tests/syn/geometry_msgs.syn
  synapse --lang c -o generated synapse-integration-tests/syn/geometry_msgs.syn
  synapse generate --lang rust -o generated synapse-integration-tests/syn/geometry_msgs.syn
  synapse generate --lang c -o generated --single-file synapse-integration-tests/syn/geometry_msgs.syn")]
struct GenerateArgs {
    /// Target language to generate.
    #[arg(long, value_enum)]
    lang: Option<CliLang>,

    /// Write output to this directory instead of stdout.
    /// By default, this writes the root file plus all transitive imports.
    #[arg(long, short = 'o')]
    out_dir: Option<PathBuf>,

    /// With --out-dir, generate only the requested root file.
    #[arg(long)]
    single_file: bool,

    /// Root .syn file to generate from.
    file: Option<PathBuf>,
}

#[derive(Clone, ValueEnum)]
enum CliLang {
    /// NASA cFS C header (.h)
    C,
    /// Rust #[repr(C)] bindings (.rs)
    Rust,
}

#[derive(Clone, ValueEnum)]
enum CliRegistryFormat {
    /// JSON packet registry.
    Json,
    /// CSV packet registry.
    Csv,
}

impl From<CliLang> for cfs_synapse::Lang {
    fn from(value: CliLang) -> Self {
        match value {
            CliLang::C => cfs_synapse::Lang::C,
            CliLang::Rust => cfs_synapse::Lang::Rust,
        }
    }
}

impl From<CliRegistryFormat> for cfs_synapse::RegistryFormat {
    fn from(value: CliRegistryFormat) -> Self {
        match value {
            CliRegistryFormat::Json => cfs_synapse::RegistryFormat::Json,
            CliRegistryFormat::Csv => cfs_synapse::RegistryFormat::Csv,
        }
    }
}

pub(crate) fn run() {
    let args = Args::parse();
    match args.command {
        Some(Command::Check(check)) => check_path(check),
        Some(Command::Doc(doc)) => doc_path(doc),
        Some(Command::Generate(generate)) => generate_path(generate),
        Some(Command::Registry(registry)) => registry_path(registry),
        None => generate_path(args.generate),
    }
}

fn check_path(args: CheckArgs) {
    cfs_synapse::check_paths(&args.files).unwrap_or_else(|e| {
        eprintln!("Error checking inputs:\n{e}");
        process::exit(1);
    });
    for file in args.files {
        eprintln!("checked {}", file.display());
    }
}

fn doc_path(args: DocArgs) {
    match args.out_dir {
        None => {
            let output = cfs_synapse::generate_docs(&args.files).unwrap_or_else(|e| {
                eprintln!("Error documenting inputs:\n{e}");
                process::exit(1);
            });
            print!("{output}");
        }
        Some(dir) => {
            let out_path = cfs_synapse::write_docs(&args.files, &dir).unwrap_or_else(|e| {
                eprintln!("Error documenting inputs:\n{e}");
                process::exit(1);
            });
            eprintln!("wrote {}", out_path.display());
        }
    }
}

fn registry_path(args: RegistryArgs) {
    let format = cfs_synapse::RegistryFormat::from(args.format);
    match args.output {
        None => {
            let output = cfs_synapse::generate_registry(&args.files, format).unwrap_or_else(|e| {
                eprintln!("Error exporting registry:\n{e}");
                process::exit(1);
            });
            print!("{output}");
        }
        Some(path) => {
            let out_path =
                cfs_synapse::write_registry(&args.files, &path, format).unwrap_or_else(|e| {
                    eprintln!("Error exporting registry:\n{e}");
                    process::exit(1);
                });
            eprintln!("wrote {}", out_path.display());
        }
    }
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
