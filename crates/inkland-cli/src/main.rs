//! The `inkland` command line tool.
//!
//! The tool parses the flags, calls the library, and prints JSON on stdout.
//! It has no map logic of its own. Logs go to stderr, and only with
//! `--verbose`. The tool never asks a question.

use clap::{Args, Parser, Subcommand};
use inkland::{Error, ErrorCode, ExitCode, Override, exit_code_of};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = "inkland",
    version,
    about = "A deterministic fantasy map engine."
)]
struct Cli {
    /// Write logs to stderr.
    #[arg(long, global = true)]
    verbose: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate the maps of a config.
    Generate(GenerateArgs),
    /// Print the JSON Schema of the config.
    Schema,
    /// Print facts about one point of a world.
    Query {
        #[arg(long)]
        world: PathBuf,
        /// The point `x,y`, in config units.
        #[arg(long)]
        at: String,
    },
    /// Export a world as GeoJSON.
    Export {
        #[arg(long)]
        world: PathBuf,
        #[arg(long)]
        format: String,
        #[arg(long)]
        layers: Option<String>,
    },
    /// Apply an edit script to a world.
    Edit {
        #[arg(long)]
        world: PathBuf,
        #[arg(long)]
        script: PathBuf,
        #[arg(short = 'o', long = "out")]
        out: PathBuf,
    },
    /// Run the print and label checks on an SVG file.
    Check {
        #[arg(long)]
        svg: PathBuf,
    },
    /// Print the version and the third-party notices.
    Version {
        /// Accepted for clarity. The output is always JSON.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Args)]
struct GenerateArgs {
    /// The config file.
    #[arg(long)]
    config: PathBuf,
    /// The output directory.
    #[arg(long, default_value = "out")]
    out_dir: PathBuf,
    /// Also write a PNG preview to this file.
    #[arg(long)]
    preview: Option<PathBuf>,
    /// Render only this variant.
    #[arg(long)]
    variant: Option<String>,
    /// Check the config and the pins. Generate nothing and write nothing.
    #[arg(long)]
    dry_run: bool,
    /// Replace `seed`.
    #[arg(long)]
    seed: Option<u64>,
    /// Replace `world.cells`.
    #[arg(long)]
    cells: Option<u32>,
    /// Replace one value: `POINTER=JSON`, for example `/world/cells=20000`.
    /// Repeatable. Applied after --seed and --cells.
    #[arg(long = "set", value_name = "POINTER=JSON")]
    set: Vec<String>,
}

fn main() {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            use clap::error::ErrorKind;
            if matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) {
                print!("{e}");
                std::process::exit(0);
            }
            let msg = e.to_string();
            let first = msg.lines().next().unwrap_or("invalid command line");
            let first = first.trim_start_matches("error: ").to_owned();
            fail(vec![Error::new(ErrorCode::CliUsage, "", first)]);
        }
    };
    let verbose = cli.verbose;
    let out = match cli.command {
        Command::Generate(args) => generate(args, verbose),
        Command::Schema => Ok(inkland::schema()),
        Command::Version { .. } => Ok(inkland::version_info()),
        Command::Query { .. } => not_yet("query", 10),
        Command::Export { .. } => not_yet("export", 10),
        Command::Edit { .. } => not_yet("edit", 10),
        Command::Check { .. } => not_yet("check", 7),
    };
    match out {
        Ok(value) => {
            print_json(&value);
            // The output is written, but a check failed.
            if value["checks_failed"] == true {
                std::process::exit(ExitCode::ChecksFailed.code());
            }
            std::process::exit(ExitCode::Ok.code());
        }
        Err(errors) => fail(errors),
    }
}

fn generate(args: GenerateArgs, verbose: bool) -> Result<Value, Vec<Error>> {
    let text = std::fs::read_to_string(&args.config).map_err(|e| {
        vec![
            Error::new(
                ErrorCode::IoRead,
                "",
                format!("cannot read the config: {e}"),
            )
            .with("file", args.config.display().to_string()),
        ]
    })?;
    let mut overrides = Vec::new();
    if let Some(seed) = args.seed {
        overrides.push(Override {
            pointer: "/seed".into(),
            value: seed.into(),
        });
    }
    if let Some(cells) = args.cells {
        overrides.push(Override {
            pointer: "/world/cells".into(),
            value: cells.into(),
        });
    }
    for s in &args.set {
        overrides.push(Override::parse(s).map_err(|e| vec![e])?);
    }
    let config = inkland::parse_config(&text, &overrides).map_err(|e| vec![e])?;
    if let Some(name) = &args.variant
        && !config.variants.iter().any(|v| &v.name == name)
    {
        return Err(vec![Error::new(
            ErrorCode::RefUnknown,
            "/variants",
            format!("there is no variant {name:?}"),
        )]);
    }
    if verbose {
        eprintln!("inkland: config read, {} variants", config.variants.len());
    }
    if args.dry_run {
        let report = inkland::dry_run(&config)?;
        if verbose {
            eprintln!("inkland: dry run passed {} checks", report.checks.len());
        }
        return Ok(serde_json::to_value(report).expect("a report always converts to JSON"));
    }
    let out = inkland::generate(&config, args.variant.as_deref())?;
    write_output(&out, &config, &args, verbose)
}

/// The longer side of the PNG preview, in pixels.
const PREVIEW_PX: u32 = 1024;

/// Write the files of `generate`. Return the summary for stdout.
fn write_output(
    out: &inkland::Output,
    config: &inkland::Config,
    args: &GenerateArgs,
    verbose: bool,
) -> Result<Value, Vec<Error>> {
    let write = |path: &Path, bytes: &[u8]| -> Result<(), Vec<Error>> {
        std::fs::write(path, bytes).map_err(|e| {
            vec![
                Error::new(ErrorCode::IoWrite, "", format!("cannot write a file: {e}"))
                    .with("file", path.display().to_string()),
            ]
        })?;
        if verbose {
            eprintln!("inkland: wrote {}", path.display());
        }
        Ok(())
    };
    std::fs::create_dir_all(&args.out_dir).map_err(|e| {
        vec![
            Error::new(
                ErrorCode::IoWrite,
                "",
                format!("cannot make the output directory: {e}"),
            )
            .with("file", args.out_dir.display().to_string()),
        ]
    })?;
    let world_path = args.out_dir.join("world.json");
    write(&world_path, out.world_json.as_bytes())?;
    let mut variants = Vec::new();
    for v in &out.variants {
        let svg_path = args.out_dir.join(format!("{}.svg", v.name));
        let report_path = args.out_dir.join(format!("{}.report.json", v.name));
        write(&svg_path, v.svg.as_bytes())?;
        let mut report_value = v.report.clone();
        let mut entry = json!({
            "checks_failed": v.checks_failed(),
            "name": v.name,
            "svg": svg_path.display().to_string(),
            "report": report_path.display().to_string(),
        });
        if let Some(preview) = &args.preview {
            let path = if out.variants.len() == 1 {
                preview.clone()
            } else {
                let stem = preview.file_stem().unwrap_or_default().to_string_lossy();
                preview.with_file_name(format!("{stem}-{}.png", v.name))
            };
            let (png, warnings) = inkland::preview_png(&v.svg, PREVIEW_PX, config)
                .map_err(|e| vec![Error::new(ErrorCode::Internal, "", e)])?;
            write(&path, &png)?;
            entry["preview"] = Value::from(path.display().to_string());
            if let Some(list) = report_value["warnings"].as_array_mut() {
                for w in warnings {
                    list.push(serde_json::to_value(w).expect("a warning converts to JSON"));
                }
            }
            entry["warnings"] = report_value["warnings"].clone();
        }
        let mut report =
            serde_json::to_string_pretty(&report_value).expect("a report converts to JSON");
        report.push('\n');
        write(&report_path, report.as_bytes())?;
        variants.push(entry);
    }
    Ok(json!({
        "world": world_path.display().to_string(),
        "variants": variants,
        "checks_failed": out.variants.iter().any(|v| v.checks_failed()),
    }))
}

fn not_yet(command: &str, milestone: u32) -> Result<Value, Vec<Error>> {
    Err(vec![Error::new(
        ErrorCode::NotImplemented,
        "",
        format!("the {command} command comes in milestone {milestone}"),
    )])
}

fn print_json(value: &Value) {
    let text = serde_json::to_string_pretty(value).expect("JSON always converts to text");
    println!("{text}");
}

fn fail(errors: Vec<Error>) -> ! {
    let code = exit_code_of(&errors);
    let first = errors.first().cloned();
    print_json(&json!({ "error": first, "errors": errors }));
    std::process::exit(code.code());
}
