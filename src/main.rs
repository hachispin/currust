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

    let bytes: Vec<u8> = fs::read(&args.cursor_file).into_diagnostic()?;
    let cursor_file_repr = &args.cursor_file.to_string_lossy();

    if &bytes[0..4] != CUR_MAGIC {
        throw!(BlobError::new(&bytes[0..4], cursor_file_repr));
    }

    let num_images = u16::from_le_bytes(bytes[4..6].try_into().into_diagnostic()?);
    info!("idCount={num_images}");

    let icon_dir_bytes: &[u8; 16] = bytes[6..22].try_into().into_diagnostic()?;
    let icon_dir = IconDirEntry::read_from_bytes(icon_dir_bytes);

    info!("Parsed ICONDIRENTRY: {icon_dir:?}");

    Ok(())
}
