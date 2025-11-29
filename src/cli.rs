//! Stores the CLI through [`Args`], which uses [`clap`].
//!
//! Other modules should use [`ParsedArgs`], which is validated.

use crate::errors::ArgError;

use std::path::PathBuf;

use clap::Parser;
use miette::{ErrReport, IntoDiagnostic, Result};
use simplelog::Level;

/// Represents received CLI arguments.
#[derive(Parser, Debug)]
#[command(version, about, long_about=None)]
pub struct Args {
    /// The path to a Windows cursor or a directory of cursors
    #[arg(value_name = "PATH")]
    cursor_paths: String,

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
    pub cursor_paths: Vec<PathBuf>,
    /// path to place converted (x)cursor
    pub out: PathBuf,
    /// if `true`, all logs are disabled
    pub quiet: bool,
    /// used as [`simplelog::LevelFilter`]
    pub log_level: Level,
    /// if `--log-file` flag is used, logs are written here
    pub log_file: Option<PathBuf>,
}

/// Validates the given `cursor_path_str` and
/// returns a vector of all valid cursors' paths.
///
/// No valid cursors being found is considered an error,
/// so the returned vector includes at least one path.
fn validate_cursor_path(cursor_path_str: &str) -> Result<Vec<PathBuf>> {
    let cursor_path = PathBuf::from(cursor_path_str)
        .canonicalize()
        .map_err(|_| ArgError::path_doesnt_exist(None, cursor_path_str))?;

    // If the input is a single file,
    if cursor_path.is_file() {
        let cursor_file_ext = cursor_path.extension().ok_or_else(|| {
            ErrReport::from(ArgError::invalid_file_ext(
                None,
                cursor_path_str,
                None,
                "cur",
            ))
        })?;

        if cursor_file_ext != "cur" {
            throw!(ArgError::invalid_file_ext(
                None,
                cursor_path_str,
                cursor_file_ext.to_str(),
                "cur",
            ));
        }

        return Ok(vec![cursor_path]);
    }

    // If the input is a directory,
    let files = cursor_path.read_dir().into_diagnostic()?;
    let mut cursor_paths = Vec::new();

    for f in files {
        let f = f.into_diagnostic()?.path();

        match f.extension() {
            Some(v) if v == "cur" => cursor_paths.push(f),
            _ => (),
        }
    }

    if cursor_paths.is_empty() {
        throw!(ArgError::no_valid_files_in_dir(None, cursor_path_str));
    }

    Ok(cursor_paths)
}

/// Validates the given `args`, this includes:
///
/// - validating input files exist and are valid (e.g, ending in `.cur`)
/// - converting types (e.g, from [`String`] to [`PathBuf`]) for construction of [`ParsedArgs`]
/// - resolving paths
///
/// ## Errors
///
/// Errors can occur for a multitude of reasons, which include:
///
/// - a path is to a directory with no valid (cursor) files
/// - the path provided doesn't exist at all
/// - the path is to a file with no `.cur` extension
///
/// Most errors here are covered under [`ArgError`].
pub fn validate_args(args: Args) -> Result<ParsedArgs> {
    let cursor_files = validate_cursor_path(&args.cursor_paths)?;

    let out = PathBuf::from(&args.out)
        .canonicalize()
        .map_err(|_| ArgError::path_doesnt_exist(Some("-o or --out"), &args.out))?;

    // map `Option<String>` to Option<PathBuf>, canonicalizing `Some(PathBuf)`
    let log_file = args
        .log_file
        .map(|s| {
            PathBuf::from(&s)
                .canonicalize()
                .map_err(|_| ArgError::path_doesnt_exist(Some("--log-file"), &s))
        })
        .transpose()?;

    Ok(ParsedArgs {
        cursor_paths: cursor_files,
        out,
        quiet: args.quiet,
        log_level: args.log_level,
        log_file,
    })
}
