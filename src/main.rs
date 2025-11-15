use std::{fs, path::PathBuf};

use clap::Parser;
use log::{self, debug, info};
use miette::{IntoDiagnostic, Result};
use simplelog::Level;

/// Represents received CLI arguments.
#[derive(Parser, Debug)]
#[command(version, about, long_about=None)]
struct Args {
    /// The path to the Windows cursor
    #[arg(value_name = "FILE", help_heading = "d")]
    cursor_file: String,

    /// Disables logs entirely
    #[arg(short, long)]
    quiet: bool,

    /// Shows all logs with severity ≥ LEVEL
    #[arg(long, value_name = "LEVEL", default_value_t = Level::Warn)]
    log_level: Level,

    /// Writes logs to FILE instead of the terminal
    #[arg(long, value_name = "FILE")]
    log_file: Option<String>,
}

/// [`Args`] as a pure data aggregate.
///
/// Some fields like [`Args::log_file`] are made to be more
/// type correct here ([`PathBuf`] rather than [`String`])
#[allow(unused)]
#[derive(Debug)]
struct ParsedArgs {
    cursor_file: PathBuf,
    quiet: bool,
    log_level: Level,
    log_file: Option<PathBuf>,
}

/// Validates the given `args`
fn validate_args(args: Args) -> Result<ParsedArgs> {
    let cursor_file = PathBuf::from(args.cursor_file)
        .canonicalize()
        .into_diagnostic()?;

    let cursor_file_ext = cursor_file.extension().expect(&format!(
        "failed to parse extension for cursor input {:?}",
        cursor_file
    ));

    if cursor_file_ext != "cur" {
        return Err(miette::miette!(
            "expected extension `.cur`, got {}",
            cursor_file_ext.display()
        ));
    }

    let log_file = args.log_file.map(|s| PathBuf::from(s));

    if let Some(p) = &log_file {
        info!("Creating log file");
        fs::create_dir_all(p).into_diagnostic()?;
        fs::write(p, "").into_diagnostic()?;
    }

    Ok(ParsedArgs {
        cursor_file,
        quiet: args.quiet,
        log_level: args.log_level,
        log_file,
    })
}

fn main() -> Result<()> {
    let args = Args::parse();
    debug!("raw_args={args:?}");
    let args = validate_args(args)?;

    println!("{args:?}");

    Ok(())
}
