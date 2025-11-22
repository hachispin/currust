use currust::{
    cli::{Args, validate_args},
    logging::init_logging,
    models::{CursorImage, WinCursor},
};

use log::debug;

use image::{RgbaImage, ImageBuffer};
use clap::Parser;
use miette::{IntoDiagnostic, Result};

fn main() -> Result<()> {
    miette::set_panic_hook();
    let args = validate_args(Args::parse())?;
    init_logging(&args)?;
    debug!("args={args:?}");

    let cur = WinCursor::new(&args.cursor_file)?;
    let cur_image = CursorImage::from_win_cur(cur)?;

    for i in cur_image {
        let img: RgbaImage = ImageBuffer::from_raw(
            i.width, i.height, i.rgba
        ).unwrap();

        img.save(&args.out.join("image.png")).into_diagnostic()?;
    }

    Ok(())
}
