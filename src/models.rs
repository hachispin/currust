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
#[br(
    little,
    assert(_reserved == 0),
    assert(hotspot_x <= width as u16),
    assert(hotspot_y <= height as u16)
)]
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
    /// note that for `.bmp` specifically, this leads you to `BITMAPINFOHEADER`
    pub image_offset: u32,
}

/// Models the byte layout of a `BITMAPINFOHEADER`. This is needed
/// for parsing `.bmp` files in memory, used in the `.cur` format.
///
/// More specifically, this is for DIBs. (device independent bitmaps).
///
/// References:
///
/// - <https://en.wikipedia.org/wiki/BMP_file_format#DIB_header_(bitmap_information_header)>
/// - <https://learn.microsoft.com/en-us/previous-versions/ms969901(v=msdn.10)>
///
/// ( _find the "BITMAPINFOHEADER" tables!_ )
#[derive(BinRead, Debug)]
#[br(
    little,
    assert(header_size == 40),
    assert(color_planes == 1),
    assert([1, 4, 8, 24].contains(&bits_per_pixel))
)]
pub struct BitmapInfoHeader {
    /// size of the header itself in bytes
    header_size: u32,
    /// (signed) bitmap width in pixels
    width: i32,
    /// (signed) bitmap height in pixels
    height: i32,
    /// number of color planes (must be 1)
    color_planes: u16,
    /// the color depth; must be 1, 4, 8, and 24
    bits_per_pixel: u16,
    /// type of compression being used on image
    compression_method: CompressionMethod,
    /// size of raw bitmap data, if 0, use [`Self::image_size_default`]
    image_size: u32,

    /// default calculated size. this value **should only
    /// be used if `image_size` is set to 0**
    /// 
    /// explanation can be found here:
    /// <https://learn.microsoft.com/en-us/previous-versions/ms969901(v=msdn.10)#overview>
    #[br(
        calc = (((((width * bits_per_pixel as i32) + 31) & !31) >> 3) * height)
        .try_into().unwrap())
    ]
    image_size_default: u32,

    /// (signed) horizontal resolution of image (pixel per metre)
    horizontal_ppm: i32,
    /// (signed) vertical resolution of image (pixel per metre)
    vertical_ppm: i32,
    /// number of colors in color palette, if 0, use [`Self::color_count_default`]
    color_count: u32,

    /// default color count. **should only be used
    /// if [`Self::color_count`] is set to 0**
    /// 
    /// ref: <https://en.wikipedia.org/wiki/BMP_file_format#Color_table>
    #[br(calc = 2u32.pow(bits_per_pixel as u32))]
    color_count_default: u32,

    /// number of "important" colors used; generally useless
    imp_color_count: u32,
}

/// A field in `BITMAPINFOHEADER` used to specify
/// the compression method used for its image.
///
/// Reference: <https://en.wikipedia.org/wiki/BMP_file_format#DIB_header_(bitmap_information_header)>
///
/// ( _find the "Compression method" table!_ )
#[derive(BinRead, Debug)]
#[br(repr = u32)]
enum CompressionMethod {
    RGB = 0,
    RLE8 = 1,
    RLE4 = 2,
    BITFIELDS = 3,
    JPEG = 4,
    PNG = 5,
    ALPHABITFIELDS = 6,
    CMYK = 11,
    CMYKRLE8 = 12,
    CMYKRLE4 = 13,
}

/// A (hotspot/click-pixel/whatever)'s coordinates.
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
    pub fn extract_images(&self) -> Vec<BitmapInfoHeader> {
        let mut images = Vec::with_capacity(self.icon_dir.entries.len());

        for entry in &self.icon_dir.entries {
            let size = entry.image_size as usize;
            let offset = entry.image_offset as usize;

            let image_blob = &self.blob[offset..(offset + size)];

            images.push(BitmapInfoHeader::read(&mut Cursor::new(image_blob)).unwrap());
        }

        return images;
    }
}
