//! Stores the CLI through [`Args`], which uses [`clap`].
//!
//! Other modules should use [`ParsedArgs`], which is validated.

use crate::errors::ArgError;

use std::path::PathBuf;

use clap::Parser;
use miette::{ErrReport, Result};
use simplelog::Level;

/// Represents received CLI arguments.
#[derive(Parser, Debug)]
#[command(version, about, long_about=None)]
pub struct Args {
    /// The path to the Windows cursor
    #[arg(value_name = "FILE")]
    cursor_file: String,

    /// The directory to place the converted cursor
    #[arg(short, long, value_name = "DIR", default_value_t = String::from("./"))]
    out: String,

    /// Disables logs entirely
    #[arg(short, long, help_heading = "Logging")]
    quiet: bool,

    /// Shows all logs with severity ≥ LEVEL
    ///
    /// Available severity levels: TRACE, DEBUG, INFO, WARN, ERROR
    #[arg(long, value_name = "LEVEL", default_value_t = Level::Warn, help_heading = "Logging")]
    log_level: Level,

    /// Writes logs to FILE instead of the terminal
    #[arg(long, value_name = "FILE", help_heading = "Logging")]
    log_file: Option<String>,
}

/// [`Args`] as a pure data aggregate.
///
/// Some fields like [`Args::log_file`] are made to be more
/// type correct here ([`PathBuf`] rather than [`String`])
#[allow(unused)]
#[derive(Debug)]
pub struct ParsedArgs {
    /// path to Windows cursor to be converted
    pub cursor_file: PathBuf,
    /// path to place converted (x)cursor
    pub out: PathBuf,
    /// if `true`, all logs are disabled
    pub quiet: bool,
    /// used as [`simplelog::LevelFilter`]
    pub log_level: Level,
    /// if `--log-file` flag is used, logs are written here
    pub log_file: Option<PathBuf>,
}

/// Validates the given `args`, this includes:
///
/// - validating input files exist and are valid (e.g, ending in `.cur`)
/// - converting types (e.g, from [`String`] to [`PathBuf`])
///      for construction of [`ParsedArgs`]
/// - resolving paths
///
pub fn validate_args(args: Args) -> Result<ParsedArgs> {
    let cursor_file = PathBuf::from(&args.cursor_file)
        .canonicalize()
        .map_err(|_| ArgError::missing_file(None, &args.cursor_file))?;

    let cursor_file_ext = cursor_file.extension().ok_or_else(|| {
        ErrReport::from(ArgError::invalid_file_ext(
            None,
            &args.cursor_file,
            None,
            "cur",
        ))
    })?;

    let out = PathBuf::from(&args.out)
        .canonicalize()
        .map_err(|_| ArgError::missing_file(Some("-o or --out"), &args.out))?;

    // this isn't comprehensive--file headers are validated later
    if cursor_file_ext != "cur" {
        return Err(ErrReport::from(ArgError::invalid_file_ext(
            None,
            &args.cursor_file,
            Some(&cursor_file_ext.to_string_lossy()),
            "cur",
        )));
    }

    // map `Option<String>` to Option<PathBuf>, canonicalizing `Some(PathBuf)`
    let log_file = args
        .log_file
        .map(|s| {
            PathBuf::from(&s)
                .canonicalize()
                .map_err(|_| ArgError::missing_file(Some("--log-file"), &s))
        })
        .transpose()?;

    Ok(ParsedArgs {
        cursor_file,
        out,
        quiet: args.quiet,
        log_level: args.log_level,
        log_file,
    })
}
