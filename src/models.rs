//! Contains structs modeling byte layouts of Windows cursors.
//!
//! Note that the `.cur` format follows little-endian byte ordering.

use std::{fs, io::Cursor, path::Path};

use binrw::BinRead;
use miette::{IntoDiagnostic, Result};

/// Models the byte layout of `ICONDIR`.
///
/// Reference: <https://en.wikipedia.org/wiki/ICO_(file_format)#ICONDIR_structure>
#[derive(BinRead, Debug)]
#[br(little, magic = b"\x00\x00\x02\x00")] // contains reserved and type
pub struct IconDir {
    // `idReserved` and `idType` aren't here as
    // they're considered part of the magic bytes.
    // `binrw` starts reading *after* them.
    /// the number of images
    pub num_images: u16,
    /// entries exist for each image, containing
    /// info such as hotspot coordinates
    #[br(count=num_images)]
    pub entries: Vec<IconDirEntry>,
}

/// Models the byte layout of `ICONDIRENTRY`, which
/// stores info regarding an image (may be bmp/png)
///
/// Reference: <https://en.wikipedia.org/wiki/ICO_(file_format)#ICONDIRENTRY_structure>
#[derive(BinRead, Debug)]
#[brw(little, assert(_reserved == 0), assert(hotspot_x <= width as u16), assert(hotspot_y <= height as u16))]
pub struct IconDirEntry {
    /// width of stored image
    width: u8,
    // height of stored image
    height: u8,
    /// number of colors in palette; 0 if not used
    color_count: u8,
    /// must be 0
    _reserved: u8,
    /// horizontal coordinates from left in pixels
    /// for cursor click pizel (i.e, hotspot)
    pub hotspot_x: u16,
    /// vertical coordinates from top in pixels
    /// for cursor click pizel (i.e, hotspot)
    pub hotspot_y: u16,
    /// image data size in bytes
    image_size: u32,
    /// offset of image data (png/bmp)
    /// from beginning of `.cur` file
    pub image_offset: u32,
}

/// Stores [`IconDir`], along with its corresponding `.cur` blob.
///
/// I could definitely refactor away some redundant
/// copies here and there, but it's far from a priority.
#[derive(Debug)]
pub struct WinCursor {
    /// raw owned bytes used for reading stored images
    blob: Vec<u8>,
    /// contains structured metadata
    pub icon_dir: IconDir,
}

impl WinCursor {
    /// Creates a new [`WinCursor`].
    pub fn new(cur: &Path) -> Result<Self> {
        let bytes = fs::read(cur).into_diagnostic()?;
        let icon_dir = IconDir::read(&mut Cursor::new(&bytes)).into_diagnostic()?;

        Ok(Self {
            blob: bytes,
            icon_dir,
        })
    }
}
