use std::{fs::File, io::BufWriter};

use currust::{
    cli::{Args, validate_args},
    cursors::{common::CursorImage, windows::WinCursor},
    logging::init_logging,
};

use clap::Parser;
use log::debug;
use miette::{IntoDiagnostic, Result};
use png::{BitDepth, ColorType, Encoder};

fn main() -> Result<()> {
    miette::set_panic_hook();
    let args = validate_args(Args::parse())?;
    init_logging(&args)?;
    debug!("args={args:?}");

    for (i, cursor_path) in args.cursor_paths.into_iter().enumerate() {
        let cur = WinCursor::new(&cursor_path)?;
        let cursor_images = CursorImage::from_win_cur(cur)?;

        for (j, cursor_image) in cursor_images.into_iter().enumerate() {
            let path = args.out.join(&format!("{i}-{j}.png"));
            let file = File::create(path).into_diagnostic()?;
            let ref mut w = BufWriter::new(file);

            let mut encoder = Encoder::new(w, cursor_image.width, cursor_image.height);
            encoder.set_depth(BitDepth::Eight);
            encoder.set_color(ColorType::Rgba);

            let mut writer = encoder.write_header().into_diagnostic()?;
            writer
                .write_image_data(&cursor_image.rgba)
                .into_diagnostic()?;
        }
    }

    Ok(())
}
