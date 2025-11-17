use std::fs;

use currust::{
    cli::{Args, validate_args},
    logging::init_logging,
    models::WinCursor,
};

use log::debug;

use clap::Parser;
use miette::{IntoDiagnostic, Result};

fn main() -> Result<()> {
    miette::set_panic_hook();
    let args = validate_args(Args::parse())?;
    init_logging(&args)?;
    debug!("args={args:?}");

    let cur = WinCursor::new(&args.cursor_file)?;
    debug!("cur.icon_dir={:?}", cur.icon_dir);

    todo!("Code below is non-functional!");

    let images = cur.extract_images();

    for (i, image) in images.iter().enumerate() {
        let p = args.out.join(format!("{i}"));
        fs::write(p, &image.blob).into_diagnostic()?;
    }

    Ok(())
}
