use currust::{
    cli::{Args, ParsedArgs},
    cursors::generic_cursor::{GenericCursor, ScalingType::*},
};

use anyhow::{Context, Result};
use clap::Parser;

fn main() -> Result<()> {
    let raw_args = Args::parse();
    let args = ParsedArgs::from_args(&raw_args)?;

    for cur_path in &args.cur_paths {
        let filename = cur_path.file_stem().unwrap();
        let out = args.out.join(filename);

        println!("Parsing {}", filename.display());
        let mut cursor = GenericCursor::from_cur_path(cur_path)?;

        cursor.add_scale(4, Downscale)?;
        cursor.add_scale(2, Downscale)?;
        cursor.add_scale(2, Upscale)?;
        cursor.add_scale(3, Upscale)?;

        println!("cursor.images.len()={}", cursor.base_images().len());

        cursor.save_as_xcursor(out).context(
            "\
            An error occured while converting to Xcursor!\n\
            Any produced Xcursor files may be corrupted.",
        )?;
    }

    Ok(())
}
