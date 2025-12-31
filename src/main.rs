use currust::{
    cli::{Args, ParsedArgs},
    cursors::{cursor_image::ScalingType::*, generic_cursor::GenericCursor},
};

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let raw_args = Args::parse();
    let args = ParsedArgs::from_args(&raw_args)?;
    let test = &args.cur_paths[0];

    let mut cursor = GenericCursor::from_ani_path(test)?;
    cursor.add_scale(2, Upscale)?;
    cursor.add_scale(3, Upscale)?;
    dbg!(&cursor.scaled_images().len());
    cursor.save_as_xcursor(args.out.join("left_ptr"))?;

    Ok(())
}
