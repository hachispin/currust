use currust::{
    cli::{Args, ParsedArgs},
    cursors::{cursor_image::ScalingType::*, generic_cursor::GenericCursor},
};

use anyhow::Result;
use clap::Parser;
use rayon::prelude::*;

fn main() -> Result<()> {
    let raw_args = Args::parse();
    let args = ParsedArgs::from_args(&raw_args)?;

    // 2. use par_iter for parallel processing
    args.cur_paths
        .par_iter()
        .try_for_each(|path| -> Result<()> {
            let filename = path.file_stem().unwrap();
            let out = args.out.join(filename);

            // parse & manipulate the cursor as before
            let mut cursor = GenericCursor::from_ani_path(path)?;
            cursor.add_scale(2, Downscale)?;
            cursor.add_scale(2, Upscale)?;
            cursor.add_scale(3, Upscale)?;
            cursor.add_scale(4, Upscale)?;
            cursor.save_as_xcursor(out)?;

            Ok(())
        })?;

    Ok(())
}
