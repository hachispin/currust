use currust::{
    cli::{Args, validate_args},
    logging::init_logging,
    models::{CursorImage, WinCursor},
};

use log::debug;

use clap::Parser;
use image::{ImageBuffer, RgbaImage};
use miette::{IntoDiagnostic, Result};

fn main() -> Result<()> {
    miette::set_panic_hook();
    let args = validate_args(Args::parse())?;
    init_logging(&args)?;
    debug!("args={args:?}");

    for (i, cursor_path) in args.cursor_paths.into_iter().enumerate() {
        let cur = WinCursor::new(&cursor_path)?;
        let cur_image = CursorImage::from_win_cur(cur)?;

        for (j, cursor_image) in cur_image.into_iter().enumerate() {
            let img: RgbaImage =
                ImageBuffer::from_raw(cursor_image.width, cursor_image.height, cursor_image.rgba)
                    .unwrap();

            img.save(&args.out.join(format!("[Cursor {i}] {j}.png")))
                .into_diagnostic()?;
        }
    }

    Ok(())
}
