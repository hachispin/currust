use currust::{
    cli::{Args, validate_args},
    cursors::{common::CursorImage, linux::Xcursor, windows::WinCursor},
    logging::init_logging,
};

use clap::Parser;
use log::debug;
use miette::Result;

fn main() -> Result<()> {
    miette::set_panic_hook();
    let args = validate_args(Args::parse())?;
    init_logging(&args)?;
    debug!("args={args:?}");

    for cursor_path in args.cursor_paths {
        let cur = WinCursor::new(&cursor_path)?;
        let cursor_images = CursorImage::from_win_cur(&cur)?;

        for cursor_image in cursor_images {
            let path = args.out.join("xcur_testing/left_ptr");
            let xcur = Xcursor::from_cursor_image(&[cursor_image]);
            xcur.save(&path)?;
        }
    }

    Ok(())
}
