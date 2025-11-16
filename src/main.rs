use std::fs;

use currust::{
    cli::{Args, validate_args},
    errors::BlobError,
    logging::init_logging,
    models::IconDirEntry,
};

use log::{debug, info};

use clap::Parser;
use miette::{ErrReport, IntoDiagnostic, Result};
use zerocopy::FromBytes;

macro_rules! throw {
    ($e:expr) => {
        return Err(ErrReport::from($e))
    };
}

// Reference: https://en.wikipedia.org/wiki/ICO_(file_format)#ICONDIR_structure
const CUR_MAGIC: [u8; 4] = [0x0, 0x0, 0x2, 0x0];

fn main() -> Result<()> {
    miette::set_panic_hook();
    let args = validate_args(Args::parse())?;
    init_logging(&args)?;
    debug!("args={args:?}");

    todo!();
}
