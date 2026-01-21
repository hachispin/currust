use currust::{
    cli::{Args, CursorPath, ParsedArgs},
    cursors::generic_cursor::GenericCursor,
};

use anyhow::Result;
use clap::Parser;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

fn parse_cursor(args: &ParsedArgs, cursor: &CursorPath) -> Result<()> {
    let filename = cursor.path.file_stem().unwrap();
    let out = args.out.join(filename);

    let mut cursor = if cursor.is_animated {
        GenericCursor::from_ani_path(&cursor.path)
    } else {
        GenericCursor::from_cur_path(&cursor.path)
    }?;

    args.scale_to.iter().try_for_each(|&sf| {
        let alg = if sf > 1.0 {
            args.upscale_with
        } else {
            args.downscale_with
        };

        cursor.add_scale(sf, alg)
    })?;

    cursor.save_as_xcursor(out)?;

    Ok(())
}

fn main() -> Result<()> {
    let raw_args = Args::parse();
    let args = ParsedArgs::from_args(raw_args)?;

    if args.use_rayon {
        args.cursor_paths
            .par_iter()
            .try_for_each(|cp| parse_cursor(&args, cp))?;
    } else {
        args.cursor_paths
            .iter()
            .try_for_each(|cp| parse_cursor(&args, cp))?;
    }

    Ok(())
}
