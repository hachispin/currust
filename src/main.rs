use currust::{
    cli::{Args, ParsedArgs},
    cursors::{generic_cursor::GenericCursor, themes::CursorTheme},
};

use anyhow::{Result, anyhow};
use clap::Parser;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::path::Path;

fn theme_pipeline(dir: &Path, args: &ParsedArgs) -> Result<()> {
    let mut theme = CursorTheme::from_theme_dir(dir)?;

    for &sf in &args.scale_to {
        theme.add_scale(sf, args.get_algorithm(sf))?;
    }

    theme.save_as_x11_theme(&args.out)
}

fn main() -> Result<()> {
    let raw_args = Args::parse();
    let args = ParsedArgs::from_args(raw_args)?;

    if args.cursor_theme_dirs.len() > 1 {
        args.cursor_theme_dirs
            .par_iter()
            .try_for_each(|d| theme_pipeline(d, &args))?;
    } else {
        args.cursor_theme_dirs
            .iter()
            .try_for_each(|d| theme_pipeline(d, &args))?;
    }

    args.cursor_files.par_iter().try_for_each(|f| {
        let mut cursor = GenericCursor::from_path(f)?;
        let filename = args.out.join(
            f.file_stem()
                .ok_or_else(|| anyhow!("no file stem for cursor_file={}", f.display()))?,
        );

        for &sf in &args.scale_to {
            cursor.add_scale(sf, args.get_algorithm(sf))?;
        }

        cursor.save_as_xcursor(filename)
    })?;

    Ok(())
}
