//! Stores the CLI through [`Args`], which uses [`clap`].
//!
//! Other modules should use [`ParsedArgs`], which
//! has validation for, e.g, filepath arguments.

use crate::errors::ArgParseError;

use std::{
    fs,
    path::{Path, PathBuf},
};

use clap::Parser;
use miette::{IntoDiagnostic, Result};
use simplelog::Level;

/// Represents received CLI arguments.
#[derive(Parser, Debug)]
#[command(version, about, long_about=None)]
pub struct Args {
    /// The path to the Windows cursor
    #[arg(value_name = "FILE")]
    cursor_file: String,

    /// Disables logs entirely
    #[arg(short, long, help_heading = "logging")]
    quiet: bool,

    /// Shows all logs with severity ≥ LEVEL
    #[arg(long, value_name = "LEVEL", default_value_t = Level::Warn, help_heading = "logging")]
    log_level: Level,

    /// Writes logs to FILE instead of the terminal
    #[arg(long, value_name = "FILE", help_heading = "logging")]
    log_file: Option<String>,
}

/// [`Args`] as a pure data aggregate.
///
/// Some fields like [`Args::log_file`] are made to be more
/// type correct here ([`PathBuf`] rather than [`String`])
#[allow(unused)]
#[derive(Debug)]
pub struct ParsedArgs {
    pub cursor_file: PathBuf,
    pub quiet: bool,
    pub log_level: Level,
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
        .map_err(|_| ArgParseError::invalid_file(None, &args.cursor_file))?;

    let cursor_file_ext = cursor_file.extension().ok_or_else(|| {
        miette::miette!(
            "failed to parse extension for cursor input {:?}",
            cursor_file
        )
    })?;

    // this isn't comprehensive--file headers are validated later
    if cursor_file_ext != "cur" {
        return Err(miette::miette!(
            "expected extension `.cur`, got {}",
            cursor_file_ext.display()
        ));
    }

    let log_file = args.log_file.map(|s| PathBuf::from(s));

    if let Some(p) = &log_file {
        if p == Path::new("/") {
            return Err(miette::miette!("cannot create log at file-system root"));
        }

        fs::create_dir_all(p.parent().unwrap()).into_diagnostic()?;
        fs::write(p, "").into_diagnostic()?;
    }

    Ok(ParsedArgs {
        cursor_file,
        quiet: args.quiet,
        log_level: args.log_level,
        log_file,
    })
}
