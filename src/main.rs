use currust::cli::{Args, ParsedArgs};

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let raw_args = Args::parse();
    let args = ParsedArgs::from_args(&raw_args)?;

    println!("Parsed args: {args:?}");

    Ok(())
}
