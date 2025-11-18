use currust::{
    cli::{Args, validate_args},
    logging::init_logging,
    models::CursorImage,
};

use log::debug;

use clap::Parser;
use miette::Result;

fn main() -> Result<()> {
    miette::set_panic_hook();
    let args = validate_args(Args::parse())?;
    init_logging(&args)?;
    debug!("args={args:?}");

    todo!();

    Ok(())
}
