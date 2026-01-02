//! Module for [`clap`] code.
//!
//! This contains the [`Args`] struct, which has the [`Parser`]
//! trait, and the [`ParsedArgs`] struct, which is just plain old data.

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use clap::Parser;

/// Raw arguments from CLI. Has the [`Parser`] trait.
#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// The path to a CUR/ANI file, or a directory containing CUR/ANI files.
    path: String,

    /// Where to place parsed Xcursors.
    ///
    /// If the provided path doesn't exist yet, this
    /// attempts to create them (including parents).
    #[arg(short, long, default_value = "./")]
    out: String,
}

/// Parsed CLI arguments.
#[derive(Debug)]
pub struct ParsedArgs {
    /// All files in the specified directory that are CUR/ANI files.
    pub cursor_paths: Vec<PathBuf>,
    /// Where to put parsed Xcursor files.
    pub out: PathBuf,
}

impl ParsedArgs {
    /// Parses `args` for types that don't implement deserializers.
    ///
    /// This may also do extra work, like extracting
    /// all paths to CUR for the provided path.
    ///
    /// ## Errors
    ///
    /// If the input path is to a directory that doesn't contain CUR or
    /// ANI files, or to a file that lacks the `.cur`/`.ani` extension.
    pub fn from_args(args: &Args) -> Result<Self> {
        let cur_paths = Self::validate_cursor_path(&args.path)?;
        let out = PathBuf::from(&args.out);
        fs::create_dir_all(&out).with_context(|| format!("failed to create out={}", args.out))?;

        Ok(Self { cursor_paths: cur_paths, out })
    }

    /// Helper function for validating [`Args::path`].
    fn validate_cursor_path(path: &str) -> Result<Vec<PathBuf>> {
        // for triage purposes
        let path_str = path.to_string();

        let path = PathBuf::from(&path)
            .canonicalize()
            .with_context(|| format!("failed to canonicalize path {path_str}"))?;

        if path.is_dir() {
            let cur_paths = Self::extract_cursors(&path)?;

            if !cur_paths.is_empty() {
                return Ok(cur_paths);
            }

            bail!("no CUR files found in {path_str}, note that sub-directories aren't checked");
        } else if path.is_file() {
            if let Some(ext) = path.extension()
                && (ext == "cur" || ext == "ani")
            {
                return Ok(vec![path]);
            }

            bail!("provided file {path_str} is not a CUR file");
        }

        // metadata errors are coerced to false in the `.is_*()`
        // methods. try passing `/dev/null` for instance
        bail!("couldn't coerce {path_str} as a dir or file")
    }

    /// Returns all the files in `dir` that point to
    /// "cursor" files. (files with `.cur` or `.ani` extension)
    fn extract_cursors(cursor_dir: &Path) -> Result<Vec<PathBuf>> {
        assert!(
            cursor_dir.is_dir(),
            "passed `cur_dir` to `extract_curs()` must be a dir"
        );

        let mut cursor_paths = Vec::new();
        let path_display = cursor_dir.display();
        let entries = cursor_dir
            .read_dir()
            .with_context(|| format!("failed to read entries of cur_dir={path_display}"))?;

        for entry in entries {
            let entry = entry.with_context(|| {
                format!("`entries` iterator over cur_dir={path_display} yielded bad item")
            })?;

            let entry_path = entry.path();

            if let Some(ext) = entry_path.extension()
                && (ext == "cur" || ext == "ani")
            {
                cursor_paths.push(entry_path);
            }
        }

        Ok(cursor_paths)
    }
}
