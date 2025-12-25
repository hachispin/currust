use currust::{
    cli::{Args, ParsedArgs},
    cursors::common::GenericCursor,
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

        // add a bunch of scales
        cursor.add_downscale(4)?;
        cursor.add_downscale(2)?;
        cursor.add_upscale(2)?;
        cursor.add_upscale(3)?;

        for image in cursor.images() {
            println!("{:?}", image.dimensions());
        }

        cursor.save_as_xcursor(out).context(
            "\
            An error occured while converting to Xcursor!\n\
            Any produced Xcursor files may be corrupted.",
        )?;
    }

    Ok(())
}
