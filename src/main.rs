use currust::{
    cli::{Args, ParsedArgs},
    cursors::common::GenericCursor,
};

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let raw_args = Args::parse();
    let args = ParsedArgs::from_args(&raw_args)?;

    println!("Parsed args: {args:?}");

    for cur_path in &args.cur_paths {
        let filename = cur_path.file_stem().unwrap();

        let cursor = GenericCursor::from_cur_path(cur_path)?;
        cursor.save_as_xcursor(filename)?;
    }

    println!("Success!");

    Ok(())
}
