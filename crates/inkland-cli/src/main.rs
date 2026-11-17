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
    /// Make a config from a map file of Azgaar's Fantasy Map Generator.
    Import {
        /// The `.map` file.
        #[arg(long)]
        map: PathBuf,
        /// The config file to write.
        #[arg(short = 'o', long = "out")]
        out: PathBuf,
    },
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
    /// Run the print checks on an SVG file.
    Check {
        #[arg(long)]
        svg: PathBuf,
        /// The smallest permitted line width, in points.
        #[arg(long, default_value_t = 0.5)]
        min_line_width_pt: f64,
        /// The permitted font families, separated by commas.
        #[arg(
            long,
            default_value = "Caladea,Inter,IM Fell English,IM Fell English SC"
        )]
        fonts_allowed: String,
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
        Command::Import { map, out } => import(&map, &out, verbose),
        Command::Version { .. } => Ok(inkland::version_info()),
        Command::Query { world, at } => {
            read_text(&world, "world").and_then(|t| inkland::query(&t, &at).map_err(|e| vec![e]))
        }
        Command::Export { .. } => not_yet("export", 10),
        Command::Edit { .. } => not_yet("edit", 10),
        Command::Check {
            svg,
            min_line_width_pt,
            fonts_allowed,
        } => check(&svg, min_line_width_pt, &fonts_allowed),
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

fn import(map: &Path, out: &Path, verbose: bool) -> Result<Value, Vec<Error>> {
    let bytes = std::fs::read(map).map_err(|e| {
        vec![
            Error::new(
                ErrorCode::IoRead,
                "",
                format!("cannot read the map file: {e}"),
            )
            .with("file", map.display().to_string()),
        ]
    })?;
    let import = inkland::import_map(&bytes).map_err(|e| vec![e])?;
    let mut text =
        serde_json::to_string_pretty(&import.config).expect("a config always converts to JSON");
    text.push('\n');
    std::fs::write(out, text).map_err(|e| {
        vec![
            Error::new(
                ErrorCode::IoWrite,
                "",
                format!("cannot write the config: {e}"),
            )
            .with("file", out.display().to_string()),
        ]
    })?;
    if verbose {
        eprintln!("inkland: wrote {}", out.display());
    }
    let mut summary =
        serde_json::to_value(&import.summary).expect("a summary always converts to JSON");
    summary["config"] = Value::from(out.display().to_string());
    Ok(summary)
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

fn check(path: &Path, min_line_width_pt: f64, fonts_allowed: &str) -> Result<Value, Vec<Error>> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        vec![
            Error::new(ErrorCode::IoRead, "", format!("cannot read the SVG: {e}"))
                .with("file", path.display().to_string()),
        ]
    })?;
    let fonts: Vec<String> = fonts_allowed
        .split(',')
        .map(|f| f.trim().to_owned())
        .filter(|f| !f.is_empty())
        .collect();
    let checks = inkland::check_svg_with(&text, min_line_width_pt, fonts);
    let failed = checks.iter().any(|c| c["pass"] == false);
    Ok(json!({ "svg": path.display().to_string(), "checks": checks, "checks_failed": failed }))
}

/// The text of an input file, or an `IO_READ` error.
fn read_text(path: &Path, what: &str) -> Result<String, Vec<Error>> {
    std::fs::read_to_string(path).map_err(|e| {
        vec![
            Error::new(
                ErrorCode::IoRead,
                "",
                format!("cannot read the {what}: {e}"),
            )
            .with("file", path.display().to_string()),
        ]
    })
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
