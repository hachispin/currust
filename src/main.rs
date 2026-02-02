use currust::{
    cli::{Args, ParsedArgs},
    cursors::{generic_cursor::GenericCursor, themes::CursorTheme},
};

use anyhow::Result;
use clap::Parser;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

fn main() -> Result<()> {
    let raw_args = Args::parse();
    let args = ParsedArgs::from_args(raw_args)?;

    args.cursor_theme_dirs.par_iter().try_for_each(|dir| {
        let mut theme = CursorTheme::from_theme_dir(dir)?;

        for sf in &args.scale_to {
            let algorithm = if *sf > 1.0 {
                args.upscale_with
            } else {
                args.downscale_with
            };

            theme.add_scale(*sf, algorithm)?;
        }

        theme.save_as_x11_theme(&args.out)?;

        anyhow::Ok(())
    })?;

    for f in args.cursor_files {
        let mut cursor = GenericCursor::from_path(f)?;

        for sf in &args.scale_to {
            let algorithm = if *sf > 1.0 {
                args.upscale_with
            } else {
                args.downscale_with
            };

            cursor.add_scale(*sf, algorithm)?;
            cursor.save_as_xcursor(&args.out)?;
        }
    }

    Ok(())
}
