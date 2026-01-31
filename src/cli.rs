//! Module for [`clap`] code.
//!
//! This contains the [`Args`] struct, which has the [`Parser`]
//! trait, and the [`ParsedArgs`] struct, which is just plain old data.

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use clap::{Parser, ValueEnum};
use fast_image_resize::{FilterType, ResizeAlg};

/// Raw arguments from CLI. Has the [`Parser`] trait.
#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// The path to either:
    /// - one or more cursor files
    /// - one or directories (cursor themes)
    ///
    /// Note that you can't have paths to both directories AND files.
    ///
    /// A path to a directory is implicitly taken as a cursor theme.
    /// To override this behaviour, use the "--no-theme" flag.
    ///
    /// A cursor theme is a directory that contains some related
    /// cursors and an installer file using the INF format. If
    /// this INF file isn't located within this directory, specify
    /// the file's path using the "--convert-with" argument.
    #[arg(verbatim_doc_comment)]
    path: Vec<String>,

    /// Indicates that the directory provided is NOT a theme.
    ///
    /// This means that any installer files are ignored unless explicitly
    /// provided by the "--convert-with" argument. This isn't recommended
    /// for most use-cases as it makes conversion more manual.
    #[arg(long)]
    no_theme: bool,

    /// Path to an INF file to process a cursor theme with.
    ///
    /// This isn't needed if the INF file is already
    /// within the theme directory provided.
    #[arg(long, value_name = "PATH_TO_INF")]
    convert_with: Option<String>,

    /// Forces sequential processing (i.e, no rayon usage).
    ///
    /// Sequential processing is used by default for light workloads.
    #[arg(long)]
    sequential: bool,

    /// Forces parallel processing (i.e, rayon usage).
    ///
    /// This is enabled by default when parsing a large amount of cursors.
    /// Note that this is only effective with heavier workloads and
    /// can be slower on lighter ones (e.g, parsing 25 cursors or less).
    #[arg(long)]
    parallel: bool,

    /// Uses the provided scaling algorithm.
    ///
    /// This is overridden by "--upscale-with" and "--downscale-with", if set.
    ///
    ///  algorithm  use case
    /// nearest   pixel art if scaling to integers (e.g, 2x, 3x).
    /// box       pixel art if scaling includes decimals (e.g, 1.5x, 2x, 3x).
    /// bilinear  smooth shapes, not recommended if sharpness is desired.   
    /// mitchell  general-purpose upscaling, balances smoothness and sharpness.
    /// lanczos3  general-purpose downscaling, perserves details but may cause artifacts.
    #[arg(
        long,
        default_value = "lanczos3",
        value_name = "ALGORITHM",
        verbatim_doc_comment
    )]
    scale_with: ScalingAlgorithm,

    /// Uses the provided scaling algorithm for upscaling.
    ///
    /// This algorithm overrides the "--scale-with"
    /// algorithm when upscaling, if it's provided.
    #[arg(long, value_name = "ALGORITHM")]
    upscale_with: Option<ScalingAlgorithm>,

    /// Uses the provided scaling algorithm for downscaling.
    ///
    /// This algorithm overrides the "--scale-with"
    /// algorithm when downscaling, if it's provided.
    #[arg(long, value_name = "ALGORITHM")]
    downscale_with: Option<ScalingAlgorithm>,

    /// A list of scale factors to scale the original cursor(s) to.
    ///
    /// Scale factors can be floats (decimals) e.g: 0.0, 1.0, 1.5, etc.
    /// Any negative values are considered invalid scale factors.
    ///
    /// All scaled variations and the original cursor
    /// are included in the produced Xcursor file(s).
    #[arg(long, value_parser, num_args(1..), value_name = "F64_SCALE_FACTORS")]
    scale_to: Vec<f64>,

    /// The directory to place the parsed Xcursor file(s).
    ///
    /// If the provided path doesn't exist yet, this
    /// attempts to create it, including parents.
    #[arg(short, long, default_value = "./cursors")]
    out: String,
}

/// User-facing enum for usable scaling algorithms.
#[derive(Debug, Clone, ValueEnum)]
enum ScalingAlgorithm {
    Nearest,
    Box,
    Bilinear,
    Mitchell,
    Lanczos3,
}

// not meant to be used directly; use ResizeAlg impl.
impl From<&ScalingAlgorithm> for FilterType {
    fn from(alg: &ScalingAlgorithm) -> Self {
        match alg {
            ScalingAlgorithm::Nearest => unreachable!(),
            ScalingAlgorithm::Box => Self::Box,
            ScalingAlgorithm::Bilinear => Self::Bilinear,
            ScalingAlgorithm::Mitchell => Self::Mitchell,
            ScalingAlgorithm::Lanczos3 => Self::Lanczos3,
        }
    }
}

impl From<&ScalingAlgorithm> for ResizeAlg {
    fn from(alg: &ScalingAlgorithm) -> Self {
        match alg {
            ScalingAlgorithm::Nearest => Self::Nearest,
            v => Self::Convolution(FilterType::from(v)),
        }
    }
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
    pub scale_to: Vec<f64>,
    /// Algorithm for upscaling.
    pub upscale_with: ResizeAlg,
    /// Algorithm for downscaling.
    pub downscale_with: ResizeAlg,
    /// Where to put parsed Xcursor files.
    pub out: PathBuf,
}

impl ParsedArgs {
    /// Parses `args` for types that don't implement deserializers.
    ///
    /// This may also do extra work, like extracting
    /// all paths to CUR for the provided path.
    ///
    /// ## Panics
    ///
    /// If `NaN` is somehow entered as a scale factor.
    ///
    /// ## Errors
    ///
    /// If the input path is to a directory that doesn't contain CUR or
    /// ANI files, or to a file that lacks the `.cur`/`.ani` extension.
    pub fn from_args(args: Args) -> Result<Self> {
        todo!();
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
