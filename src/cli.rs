//! Module for [`clap`] code.
//!
//! This contains the [`Args`] struct, which has the [`Parser`]
//! trait, and the [`ParsedArgs`] struct, which is just plain old data.s

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::Parser;

/// Raw arguments from CLI. Has the [`Parser`] trait.
#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// The path to a CUR file, or a directory containing CUR files.
    path: String,
}

/// Parsed CLI arguments.
#[derive(Debug)]
pub struct ParsedArgs {
    /// The path to a CUR file, or a directory that contains CUR files.
    pub path: PathBuf,
}

impl ParsedArgs {
    /// Helper function for validating [`Args::path`].
    fn validate_cur_path(path: &str) -> Result<PathBuf> {
        // for triage purposes
        let path_str = path.to_string();

        let path = PathBuf::from(&path)
            .canonicalize()
            .with_context(|| format!("failed to canonicalize path {path_str}"))?;

        // checks if a CUR file is contained (non-recursively)
        if path.is_dir() {
            let entries = path
                .read_dir()
                .with_context(|| format!("failed to read dir {path_str}"))?;

            for entry in entries {
                let entry =
                    entry.with_context(|| format!("failed to read an entry of dir {path_str}"))?;

                let entry_path = entry.path(); // binding

                // skip files with no extension
                let Some(ext) = entry_path.extension() else {
                    continue;
                };

                if ext == "cur" {
                    return Ok(path);
                }
            }

            bail!("no CUR files found in {path_str}, note that sub-directories aren't checked");
        } else if path.is_file() {
            if let Some(ext) = path.extension()
                && ext == "cur"
            {
                return Ok(path);
            }

            bail!("provided file {path_str} is not a CUR file");
        }

        // metadata errors are coerced to false in the `.is_*()`
        // methods. try passing `/dev/null` for instance
        bail!("couldn't coerce {path_str} as a dir or file")
    }

    /// Parses `args` for types that don't implement deserializers.
    ///
    /// ## Errors
    ///
    /// If the input path is to a directory that doesn't contain
    /// CUR files, or to a file that lacks the `.cur` extension.
    pub fn from_args(args: &Args) -> Result<Self> {
        let path = Self::validate_cur_path(&args.path)?;

        Ok(Self { path })
    }
}
