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
use dialoguer::Confirm;

/// Raw arguments from CLI. Has the [`Parser`] trait.
#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// The path to a CUR/ANI file, or a directory containing CUR/ANI files.
    path: String,

    /// Forces sequential processing (i.e, no `rayon`).
    ///
    /// Sequential processing is used by default for light workloads.
    #[arg(long)]
    sequential: bool,

    /// Forces `rayon` usage.
    ///
    /// This is enabled by default when parsing a large amount of cursors.
    /// Note that `rayon` is only effective with heavier workloads and
    /// can be slower on lighter ones (e.g, parsing 25 cursors or less).
    #[arg(long)]
    parallel: bool,

    /// A list of scale factors to upscale the original cursor to.
    ///
    /// All scaled variations and the original cursor
    /// are included in the produced Xcursor files.
    #[arg(long, value_parser, num_args(1..), value_name = "U32_SCALE_FACTORS")]
    upscalings: Vec<u32>,
    /// A list of scale factors to downscale the original cursor to.
    ///
    /// All scaled variations and the original cursor
    /// are included in the produced Xcursor files.
    #[arg(long, value_parser, num_args(1..), value_name = "U32_SCALE_FACTORS")]
    downscalings: Vec<u32>,

    /// Where to place parsed Xcursors.
    ///
    /// If the provided path doesn't exist yet, this
    /// attempts to create them (including parents).
    #[arg(short, long, default_value = "./")]
    out: String,
}

impl Args {
    /// The max upscaling factor for images.
    pub const MAX_UPSCALE_FACTOR: u32 = 20;
    /// The max downscaling factor for images.
    pub const MAX_DOWNSCALE_FACTOR: u32 = 5;
}

/// A path and whether if it's ANI or CUR.
#[derive(Debug)]
pub struct CursorPath {
    /// Path to ANI/CUR.
    pub path: PathBuf,
    /// If true, ANI, else CUR.
    pub is_animated: bool,
}

/// Parsed CLI arguments.
#[derive(Debug)]
pub struct ParsedArgs {
    /// All files in the specified directory that are CUR/ANI files.
    pub cursor_paths: Vec<CursorPath>,
    /// Whether to use `rayon` or not.
    pub use_rayon: bool,
    /// Scale factors.
    pub upscalings: Vec<u32>,
    /// Reciprocal scale factors.
    pub downscalings: Vec<u32>,
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
    pub fn from_args(args: Args) -> Result<Self> {
        // If the number of cursors being parsed is greater
        // than or equal to this, use `rayon` for parsing.
        const USE_RAYON_BOUND: usize = 100;

        let cursor_paths = Self::validate_cursor_path(&args.path)?;

        let use_rayon =
            (!args.sequential) && (args.parallel || cursor_paths.len() >= USE_RAYON_BOUND);

        let process_state = if use_rayon {
            "parallelly, with rayon"
        } else {
            "sequentially"
        };

        println!("processing cursors {process_state} ...");

        let out = PathBuf::from(&args.out);
        fs::create_dir_all(&out).with_context(|| format!("failed to create out={}", args.out))?;

        // deduplicate scaling factors
        let mut upscalings = args.upscalings;
        let mut downscalings = args.downscalings;

        upscalings.sort_unstable();
        downscalings.sort_unstable();
        upscalings.dedup();
        downscalings.dedup();

        if upscalings
            .iter()
            .chain(&downscalings)
            .any(|sf| [0, 1].contains(sf))
        {
            bail!("scaling factors cannot include 1 or 0");
        }

        if upscalings.iter().max() > Some(&Args::MAX_UPSCALE_FACTOR) {
            bail!(
                "max upscaling factor can't be greater than {}",
                Args::MAX_UPSCALE_FACTOR
            );
        }

        if downscalings.iter().max() > Some(&Args::MAX_DOWNSCALE_FACTOR) {
            bail!(
                "max downscaling factor can't be greater than {}",
                Args::MAX_DOWNSCALE_FACTOR
            );
        }

        if upscalings.len() >= 5
            && !Confirm::new()
                .with_prompt(
                    "you've chosen more than five upscalings, this may create large files--continue?",
                )
                .default(true)
                .interact()?
        {
            std::process::exit(0);
        }

        Ok(Self {
            cursor_paths,
            use_rayon,
            upscalings,
            downscalings,
            out,
        })
    }

    /// Helper function for validating [`Args::path`].
    fn validate_cursor_path(path: &str) -> Result<Vec<CursorPath>> {
        // for triage purposes
        let path_str = path.to_string();

        let path = PathBuf::from(&path)
            .canonicalize()
            .with_context(|| format!("failed to canonicalize path {path_str}"))?;

        if path.is_dir() {
            let cursor_paths = Self::extract_cursors(&path)?;

            if !cursor_paths.is_empty() {
                return Ok(cursor_paths);
            }

            bail!("no CUR files found in {path_str}, note that sub-directories aren't checked");
        } else if path.is_file() {
            if let Some(ext) = path.extension()
                && (ext == "cur" || ext == "ani")
            {
                return Ok(vec![CursorPath {
                    path: path.clone(),
                    is_animated: ext == "ani",
                }]);
            }

            bail!("provided file {path_str} is not a CUR file");
        }

        // metadata errors are coerced to false in the `.is_*()`
        // methods. try passing `/dev/null` for instance
        bail!("couldn't coerce {path_str} as a dir or file")
    }

    /// Returns all the files in `dir` that point to
    /// "cursor" files. (files with `.cur` or `.ani` extension)
    fn extract_cursors(cursor_dir: &Path) -> Result<Vec<CursorPath>> {
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
                // a bit fragile
                cursor_paths.push(CursorPath {
                    path: entry_path.clone(),
                    is_animated: ext == "ani",
                });
            }
        }

        Ok(cursor_paths)
    }
}
