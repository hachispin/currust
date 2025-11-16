use currust::{
    cli::{Args, validate_args},
    logging::init_logging, parse::cursor::parse_cur,
};

use log::debug;

use clap::Parser;
use miette::Result;

fn main() -> Result<()> {
    miette::set_panic_hook();
    let args = validate_args(Args::parse())?;
    init_logging(&args)?;

    debug!("args={args:?}");
    parse_cur(&args.cursor_file)?;

    Ok(())
}
