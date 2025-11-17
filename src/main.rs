use currust::{
    cli::{Args, validate_args},
    logging::init_logging,
    models::WinCursor,
};

use log::debug;

use clap::Parser;
use miette::Result;

fn main() -> Result<()> {
    miette::set_panic_hook();
    let args = validate_args(Args::parse())?;
    init_logging(&args)?;
    debug!("args={args:?}");

    let cur = WinCursor::new(&args.cursor_file)?;
    debug!("cursor dump: {:?}", cur.icon_dir);

    Ok(())
}
