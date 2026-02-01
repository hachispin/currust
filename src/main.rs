use currust::{
    cli::{Args, ParsedArgs},
    cursors::{generic_cursor::GenericCursor, themes::CursorTheme},
};

use anyhow::{Result, bail};
use clap::Parser;

fn main() -> Result<()> {
    let raw_args = Args::parse();
    let args = ParsedArgs::from_args(raw_args)?;

    // NOTE: rayon not here yet...
    for theme_dir in args.cursor_theme_dirs {
        let mut theme = CursorTheme::from_theme_dir(&theme_dir)?;

        args.scale_to.iter().try_for_each(|&sf| {
            let algorithm = if sf > 1.0 {
                args.upscale_with
            } else {
                args.downscale_with
            };

            theme.add_scale(sf, algorithm, args.use_rayon)
        })?;

        theme.save_as_x11_theme(&args.out)?;
    }

    for file in args.cursor_files {
        let Some(ext) = file.extension() else {
            bail!("no extension for file={}", file.display());
        };

        let is_animated = ext == "ani";
        let mut cursor = if is_animated {
            GenericCursor::from_ani_path(&file)
        } else {
            GenericCursor::from_cur_path(&file)
        }?;

        args.scale_to.iter().try_for_each(|&sf| {
            let algorithm = if sf > 1.0 {
                args.upscale_with
            } else {
                args.downscale_with
            };

            cursor.add_scale(sf, algorithm)
        })?;

        let filename = file.file_stem().unwrap();
        cursor.save_as_xcursor(&args.out.join(filename))?;
    }

    Ok(())
}
