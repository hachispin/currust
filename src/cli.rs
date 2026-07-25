//! Module for [`clap`] code.
//!
//! This contains the [`Args`] struct, which has the [`Parser`] trait,
//! and the [`ParsedArgs`] struct, which is just plain old data.

use crate::{
    fs_utils::find_extensions_icase,
    themes::theme::{CursorMapping, CursorTheme, CursorType, TypedCursor},
    warn,
};

use std::{fs, path::PathBuf};

use anyhow::{Result, anyhow, bail};
use clap::{Parser, ValueEnum};
use fast_image_resize::{FilterType, ResizeAlg};

use dialoguer::{
    Select,
    console::{Term, style},
    theme::ColorfulTheme,
};
use documented::DocumentedVariants;

/// Raw arguments from CLI. Has the [`Parser`] trait.
#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// The paths to cursor theme installers, cursor files, directories.
    ///
    /// Supported theme installer formats include INF and CRS as of now.
    ///
    /// Cursor file paths are converted to Xcursor (named the same as the cursor file, bar
    /// extension), while directories are expanded to all the cursor files it contains
    /// (non-recursively). This acts as an alternative for shells that can't glob (e.g., cmd).
    #[arg(required = true)]
    paths: Vec<PathBuf>,

    /// Uses a manual and interactive conversion process.
    ///
    /// This is intended for when a theme installer isn't present. All provided cursor file paths will be used.
    ///
    /// Notes for usage:
    ///
    /// - You can re-select already used cursors if needed.
    /// - Person/Location Select on Windows have no equivalent on Linux, so ignore them.
    /// - You may see missing glyphs, shown as □, �, etc. This is fine,
    ///   but if you want to see them, consider downloading a nerd font.
    #[arg(long, verbatim_doc_comment)]
    manual: bool,

    /// Uses the provided scaling algorithm.
    ///
    /// This is overridden by "--upscale-with" and "--downscale-with", if set.
    ///
    /// algorithm use case
    /// nearest   pixel art if scaling to integers (e.g, 2x, 3x).
    /// box       pixel art if scaling includes decimals (e.g, 1.5x, 2x, 3x).
    /// bilinear  smooth shapes, not recommended if sharpness is desired.
    /// mitchell  general-purpose upscaling, balances smoothness and sharpness.
    /// lanczos3  general-purpose downscaling, preserves details but may cause artifacts.
    #[arg(
        long,
        default_value = "box",
        value_name = "ALGORITHM",
        verbatim_doc_comment
    )]
    scale_with: ScalingAlgorithm,

    /// Uses the provided scaling algorithm for upscaling.
    ///
    /// This algorithm overrides the "--scale-with" algorithm when upscaling, if it's provided.
    #[arg(long, value_name = "ALGORITHM")]
    upscale_with: Option<ScalingAlgorithm>,

    /// Uses the provided scaling algorithm for downscaling.
    ///
    /// This algorithm overrides the "--scale-with" algorithm when downscaling, if it's provided.
    #[arg(long, value_name = "ALGORITHM")]
    downscale_with: Option<ScalingAlgorithm>,

    /// A list of scale factors to scale the original cursor(s) to.
    ///
    /// Scale factors can be floats (decimals) e.g: 0.5, 1.5, 2.3,
    /// etc. Any negative values are considered invalid scale factors.
    ///
    /// All scaled variations and the original cursor are included in the produced Xcursor file(s).
    #[arg(long, value_parser, num_args(1..), value_name = "F64_SCALE_FACTORS")]
    scale_to: Vec<f64>,

    /// The directory to place the parsed themes/files.
    ///
    /// If the provided path doesn't exist yet, this attempts to create it, including parents.
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
    /// All installer files.
    pub installer_files: Vec<PathBuf>,
    /// All cursor files.
    pub cursor_files: Vec<PathBuf>,
    /// Installation is manual. Or not.
    pub manual: bool,
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
        let manual = args.manual;
        let mut installer_files = Vec::new();
        let mut cursor_files = Vec::new();

        for path in paths {
            let path_display = path.display();

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
                cursor_files.extend(find_extensions_icase(&path, &["cur", "ani"])?);
            } else if path.is_file() {
                let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
                    warn!("ignoring file {path_display} as it has no extension");
                    continue;
                };

                match ext.to_ascii_lowercase().as_str() {
                    "inf" | "crs" => installer_files.push(path),
                    "cur" | "ani" => cursor_files.push(path),
                    _ => warn!("ignoring file {path_display} as it is not a cursor"),
                }
            } else {
                warn!("ignoring path={path_display} as it is neither a dir or a file",);
            }
        }

        let mut scale_to = args.scale_to;

        for &sf in &scale_to {
            if sf.is_nan() || sf.is_infinite() {
                bail!("invalid sf={sf}: can't be NaN or pos/neg infinity")
            }

            if sf <= 0.1 {
                bail!("invalid sf={sf}: can't be 0.1 or less");
            }

            if sf > 100.0 {
                bail!("invalid sf={sf}: can't be greater than 100.0")
            }
        }

        scale_to.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
        scale_to.dedup();

        let (upscale_with, downscale_with) = (
            ResizeAlg::from(args.upscale_with.as_ref().unwrap_or(&args.scale_with)),
            ResizeAlg::from(args.downscale_with.as_ref().unwrap_or(&args.scale_with)),
        );

        let out = args.out;
        fs::create_dir_all(&out)?;

        Ok(Self {
            installer_files,
            cursor_files,
            manual,
            scale_to,
            upscale_with,
            downscale_with,
            out,
        })
    }

    /// Returns the appropriate algorithm for the `scale_factor`.
    #[must_use]
    pub const fn get_algorithm(&self, scale_factor: f64) -> ResizeAlg {
        if scale_factor > 1.0 {
            self.upscale_with
        } else {
            self.downscale_with
        }
    }
}

/// Asks the user a series of prompts to construct a theme manually.
///
/// This is used for when no installer file is present.
///
/// ## Errors
///
/// - any path in `cursor_paths` has no filename
/// - [`Select`] prompt fails (e.g., if user is not in a terminal)
pub(super) fn prompt_for_theme(cursor_files: &[PathBuf]) -> Result<CursorTheme> {
    let mut mappings = Vec::with_capacity(cursor_files.len());
    let mut cursor_paths_display: Vec<_> = cursor_files
        .iter()
        .map(|p| {
            p.file_name()
                .map(|f| format!("'{}' ", f.display()))
                .ok_or_else(|| anyhow!("no file name for cursor path, p={}", p.display()))
        })
        .collect::<Result<_>>()?;

    for r#type in CursorType::VARIANTS {
        let prompt = format!(
            "Select the file representing '{:?}'.\n{}",
            r#type,
            r#type.get_variant_docs()
        );

        let chosen_index = Select::with_theme(&ColorfulTheme::default())
            .items(&cursor_paths_display)
            .with_prompt(prompt)
            .default(0)
            .report(false) // can get very messy as prompts are long
            .interact()?;

        cursor_paths_display[chosen_index].push_str(&style("✓").green().to_string());

        let path = cursor_files[chosen_index].clone();
        mappings.push(CursorMapping { r#type, path });
    }

    let name = loop {
        eprint!("Enter a theme name: ");
        let theme_name = Term::stderr().read_line()?;

        // crude, but it works
        if theme_name.contains(['/', '\\']) {
            eprintln!("Theme name can't contain '/' or '\\'.");
        } else {
            break theme_name;
        }
    };

    let typed_cursors = mappings
        .into_iter()
        .map(TypedCursor::from_mapping)
        .collect::<Result<_>>()?;

    let theme = CursorTheme::new(typed_cursors, name)?;

    Ok(theme)
}
