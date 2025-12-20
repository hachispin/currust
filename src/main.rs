use currust::cli::{Args, ParsedArgs};

use clap::Parser;
use anyhow::Result;

fn main() -> Result<()> {
    let raw_args = Args::parse();
    let args = ParsedArgs::from_args(&raw_args)?;

    println!("Parsed args: {args:?}");

    Ok(())
}
