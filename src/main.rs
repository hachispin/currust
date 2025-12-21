use currust::{
    cli::{Args, ParsedArgs},
    cursors::{xcursor::save_as_xcursor, cur::read_cur},
};

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let raw_args = Args::parse();
    let args = ParsedArgs::from_args(&raw_args)?;

    println!("Parsed args: {args:?}");

    let my_cursors = read_cur(args.path)?;
    save_as_xcursor(&my_cursors, "left_ptr")?;

    Ok(())
}
