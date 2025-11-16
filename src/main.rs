use currust::{
    cli::{Args, validate_args},
    log::init_logging,
};

use log::debug;

use clap::Parser;
use miette::Result;

fn main() -> Result<()> {
    let args = validate_args(Args::parse())?;
    init_logging(&args)?;

    debug!("raw_args={args:?}");

    Ok(())
}
