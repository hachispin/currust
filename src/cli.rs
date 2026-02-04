//! Module for [`clap`] code.
//!
//! This contains the [`Args`] struct, which has the [`Parser`]
//! trait, and the [`ParsedArgs`] struct, which is just plain old data.

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Result, bail};
use clap::{Parser, ValueEnum};
use fast_image_resize::{FilterType, ResizeAlg};

/// Raw arguments from CLI. Has the [`Parser`] trait.
#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// The paths to either cursor theme directories, cursor files, or both.
    ///
    /// Cursor file paths are converted to Xcursor (named the same as the cursor file),
    /// while theme direcory paths are converted fully into an X11 theme directory.
    ///
    /// Themes are expected to contain some cursor files and a
    /// corresponding installer file that uses the INF format.
    ///
    /// To override this behaviour, use the "--no-theme" flag, which only
    /// converts the contained cursor files and ignores any INF files.
    paths: Vec<PathBuf>,

    /// Indicates that the directory provided is NOT a theme.
    ///
    /// This means that any installer files are ignored. This isn't
    /// recommended for most use-cases as it makes conversion more manual.
    #[arg(long)]
    no_theme: bool,

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

    /// The directory to place the parsed themes/files.
    ///
    /// If the provided path doesn't exist yet, this
    /// attempts to create it, including parents.
    #[arg(short, long, default_value = "./")]
    out: PathBuf,
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

/// Parsed CLI arguments.
#[derive(Debug)]
pub struct ParsedArgs {
    /// All theme directories.
    pub cursor_theme_dirs: Vec<PathBuf>,
    /// All cursor files.
    pub cursor_files: Vec<PathBuf>,
    /// Scale factors.
    pub scale_to: Vec<f64>,
    /// Algorithm for upscaling.
    pub upscale_with: ResizeAlg,
    /// Algorithm for downscaling.
    pub downscale_with: ResizeAlg,
    /// Where to put parsed Xcursor files.
    pub out: PathBuf,
}

#[allow(unused)]
impl ParsedArgs {
    /// Parses `args`.
    ///
    /// ## Panics
    ///
    /// If `NaN` is in `Args::scale_to` (should be impossible).
    ///
    /// ## Errors
    ///
    /// If any provided paths don't exist or `out` directory can't be made.
    pub fn from_args(args: Args) -> Result<Self> {
        let paths = args.paths;
        let mut cursor_theme_dirs = Vec::new();
        let mut cursor_files = Vec::new();

        for path in paths {
            let path_display = path.display();

            // yeah yeah toctou and all that. this is just for better ux
            if !path.exists() {
                // this is not my problem. https://github.com/rust-lang/rust/issues/72653
                #[cfg(windows)]
                bail!(
                    "path={path_display} doesn't exist. \n\
                    note that if you use powershell and your path looks similar to the \
                    first, convert it to the second by removing the trailing backslash: \n\
                    .\\currust.exe '.\\a path\\to a\\dir\\' -> .\\currust.exe '.\\a path\\to a\\dir'"
                );

                bail!("path={path_display} doesn't exist");
            }

            if path.is_dir() {
                cursor_theme_dirs.push(path);
            } else if path.is_file() {
                cursor_files.push(path);
            } else {
                bail!(
                    "provided path={} is neither a dir or a file",
                    path.display()
                );
            }
        }

        // we can be pretty sure NaN isn't here
        let mut scale_to = args.scale_to;
        scale_to.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
        scale_to.dedup();

        let (upscale_with, downscale_with) = (
            ResizeAlg::from(args.upscale_with.as_ref().unwrap_or(&args.scale_with)),
            ResizeAlg::from(args.downscale_with.as_ref().unwrap_or(&args.scale_with)),
        );

        let out = args.out;
        fs::create_dir_all(&out)?;

        if args.no_theme {
            for theme in cursor_theme_dirs.drain(..) {
                cursor_files.extend(Self::extract_cursors(&theme)?);
            }
        }

        Ok(Self {
            cursor_theme_dirs,
            cursor_files,
            scale_to,
            upscale_with,
            downscale_with,
            out,
        })
    }

    fn extract_cursors(dir: &Path) -> Result<Vec<PathBuf>> {
        Ok(dir
            .read_dir()?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.extension().is_some_and(|ext| {
                    ext.eq_ignore_ascii_case("ani") || ext.eq_ignore_ascii_case("cur")
                })
            })
            .collect())
    }
}
