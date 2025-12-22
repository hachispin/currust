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

    let my_cursor = GenericCursor::from_cur_path(args.path)?;
    my_cursor.save_as_xcursor("left_ptr")?;

    Ok(())
}
