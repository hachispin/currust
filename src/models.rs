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
    pub width: u8,
    /// height of stored image
    pub height: u8,
    /// number of colors in palette; 0 if not used
    ///
    /// sort of useless, so it's underscore-prefixed
    _color_count: u8,
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
    /// offset of image data (`.png`/`.bmp`) from beginning of `.cur` file
    ///
    /// note that for `.bmp` specifically, this offset leads you to where
    /// `BITMAPINFOHEADER` starts. raw data starts 40 bytes after that.
    pub image_offset: u32,
}

/// A hotspot. Or the click pixel. Or whatever else.
#[derive(Debug)]
#[allow(missing_docs)]
pub struct Hotspot {
    pub x: u16,
    pub y: u16,
}
/// A height. And a width.
#[derive(Debug)]
#[allow(missing_docs)]
pub struct Dimensions {
    pub width: u8,
    pub height: u8,
}

/// Stores an image blob, along with its dimensions and hotspot.
#[derive(Debug)]
pub struct CursorImage {
    /// raw image data
    pub blob: Vec<u8>,
    /// coordinates of hotspot
    pub hotspot: Hotspot,
    /// height and width of image
    pub dims: Dimensions,
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

    /// Parses stored `blob` using [`Self::icon_dir`], along with
    /// other relevant fields to (presumably) convert to Xcursor.
    pub fn extract_images(&self) -> Vec<CursorImage> {
        let mut images = Vec::with_capacity(self.icon_dir.entries.len());

        for entry in &self.icon_dir.entries {
            let size = entry.image_size as usize;
            let offset = entry.image_offset as usize;

            // check magic bytes at offset
            // let magic = &self.blob[offset..(offset + 4)];

            // adjust offset/size based on magic--we're skipping
            // `BITMAPINFOHEADER` to go straight to image blob
            // let (size, offset) = match magic {
            //     // png magic
            //     [0x89, 0x50, 0x4E, 0x47] => (size, offset),
            //     // `BITMAPINFOHEADER` size decl.
            //     [0x28, 0x00, 0x00, 0x00] => (size - 40, offset + 40),
            //     // unknown
            //     _ => panic!("Unexpected magic: {magic:?}"),
            // };

            let image_blob = self.blob[offset..(offset + size)].to_vec();

            let hotspot = Hotspot {
                x: entry.hotspot_x,
                y: entry.hotspot_y,
            };

            let dims = Dimensions {
                height: entry.height,
                width: entry.width,
            };

            let image = CursorImage {
                blob: image_blob,
                hotspot,
                dims,
            };

            images.push(image);
        }

        return images;
    }
}
