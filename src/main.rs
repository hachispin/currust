#![warn(
    clippy::pedantic,
    // nursery lints:
    clippy::use_self,
    clippy::or_fun_call,
    clippy::redundant_clone,
    clippy::equatable_if_let,
    clippy::needless_collect,
    // restriction lints:
    clippy::redundant_type_annotations,
    clippy::semicolon_inside_block,
    clippy::allow_attributes
)]
#![allow(
    clippy::enum_glob_use,
    reason = "when used, scope is restricted (e.g., inside functions)"
)]

pub mod cli;
pub mod cursors;
pub mod formats;
pub mod fs_utils;
pub mod themes;

/// Helper for compile-time paths for tests.
#[cfg(test)]
macro_rules! from_root {
    ($path:literal) => {
        concat!(env!("CARGO_MANIFEST_DIR"), $path)
    };
}

#[cfg(test)]
use from_root;

use crate::{
    cli::{Args, ParsedArgs, prompt_for_theme},
    cursors::generic_cursor::GenericCursor,
    themes::theme::CursorTheme,
};

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

/// A warning.
#[macro_export]
macro_rules! warn {
    ($($msg:tt)*) => {
        eprintln!(
            "{} {}", ::dialoguer::console::style("[warning]").yellow(),
            ::dialoguer::console::style(format_args!($($msg)*)).yellow()
        )
    };
}

fn main() -> Result<()> {
    let raw_args = Args::parse();
    let args = ParsedArgs::from_args(raw_args)?;

    args.installer_files.par_iter().try_for_each(|d| {
        let mut theme = CursorTheme::from_installer_file(d)
            .with_context(|| format!("while reading dir={} as theme", d.display()))?;

        for &sf in &args.scale_to {
            theme.add_scale(sf, args.get_algorithm(sf))?;
        }

        theme.save_as_x11_theme(&args.out)
    })?;

    if args.manual {
        let mut theme = prompt_for_theme(&args.cursor_files)?;

        for &sf in &args.scale_to {
            theme.add_scale(sf, args.get_algorithm(sf))?;
        }

        theme.save_as_x11_theme(&args.out)?;
    } else {
        args.cursor_files.par_iter().try_for_each(|f| {
            let mut cursor = GenericCursor::from_path(f)
                .with_context(|| format!("while reading f={} as cursor", f.display()))?;

            let filename = args.out.join(
                f.file_stem()
                    .ok_or_else(|| anyhow!("no file stem for cursor_file={}", f.display()))?,
            );

            for &sf in &args.scale_to {
                cursor.add_scale(sf, args.get_algorithm(sf))?;
            }

            cursor.save_as_xcursor(filename)
        })?;
    }

    Ok(())
}
