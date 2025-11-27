//! Contains structs modeling byte layouts of Windows cursors.
//!
//! For this module, Windows cursors
//! follow this general structure:
//!
//! [`WinCursor`] => [`IconDir`] => [`Vec<IconDirEntry>`]
//!
//! Then, for each [`IconDirEntry`], the `image_offset`
//! field is used to read its [`DeviceIndependentBitmap`].
//!
//! The DIB's [`BitmapInfoHeader`] is then used
//! by other modules to read RGBA from its blob.
//!
//! Note that the `.cur` format follows little-endian byte ordering.

use std::{fs, io::Cursor, path::Path};

use binrw::BinRead;
use log::{debug, warn};
use miette::{IntoDiagnostic, Result};

/// Models the byte layout of `ICONDIR`.
///
/// Reference: <https://en.wikipedia.org/wiki/ICO_(file_format)#ICONDIR_structure>
#[derive(BinRead, Debug)]
#[br(little, magic = b"\x00\x00\x02\x00")] // contains reserved and type
pub(super) struct IconDir {
    /// The number of images.
    pub num_images: u16,
    /// Entries exist for each image, containing
    /// cursor info such as hotspot coordinates.
    #[br(count=num_images)]
    pub entries: Vec<IconDirEntry>,
}

/// Models the byte layout of `ICONDIRENTRY`, which
/// stores info regarding an image (may be bmp/png).
///
/// Reference: <https://en.wikipedia.org/wiki/ICO_(file_format)#ICONDIRENTRY_structure>
#[derive(BinRead, Debug)]
#[br(
    little,
    assert(_reserved == 0, "Reserved field in `ICONDIR` must be 0."),
    assert(hotspot_x <= width as u16, "Hotspot (x={hotspot_x}) outside dimensions (width={width})"),
    assert(hotspot_y <= height as u16, "Hotspot (y={hotspot_y}) outside dimensions (height={width})")
)]
pub(super) struct IconDirEntry {
    /// Width of stored image.
    pub width: u8,
    /// Height of stored image.
    pub height: u8,
    /// Number of colors in the palette; 0 if not used.
    ///
    /// The color count in [`BitmapInfoHeader`] is more reliable.
    _color_count: u8,
    /// Must be 0.
    _reserved: u8,
    /// Horizontal coordinates from the left side in
    /// pixels for cursor click pizel (i.e, hotspot).
    pub hotspot_x: u16,
    /// Vertical coordinates from the top side in
    /// pixels for cursor click pizel (i.e, hotspot).
    pub hotspot_y: u16,
    /// Image data size in bytes.
    pub image_size: u32,
    /// Offset of image data from the beginning of `.cur` blob.
    ///
    /// For `.bmp`, this leads you to `BITMAPINFOHEADER`.
    pub image_offset: u32,
}

/// Aggregate for DIB structure.
#[derive(Debug)]
pub(super) struct DeviceIndependentBitmap {
    /// Raw bytes, which includes the image data and header.
    pub(super) blob: Vec<u8>,
    /// Header of [`Self::blob`].
    pub(super) header: BitmapInfoHeader,
}

/// Full representation of a Windows cursor.
#[derive(Debug)]
pub struct WinCursor {
    /// Raw bytes.
    pub(super) blob: Vec<u8>,
    /// Header for image metadata.
    pub(super) header: IconDir,
}

impl WinCursor {
    /// Reads the given path, `cur`, parsing the file as a Windows cursor.
    pub fn new(cur: &Path) -> Result<Self> {
        debug!("Creating `WinCursor`, cur={}", cur.to_string_lossy());
        let bytes = fs::read(cur).into_diagnostic()?;
        let header = IconDir::read(&mut Cursor::new(&bytes)).into_diagnostic()?;

        debug!("Parsed ICONDIR, header={header:?}");

        Ok(Self {
            blob: bytes,
            header,
        })
    }

    /// Extracts all [`DeviceIndependentBitmap`] entries from [`Self::blob`].
    pub(super) fn extract_dibs(&self) -> Result<Vec<DeviceIndependentBitmap>> {
        debug!("Extracting DIBs from entries={:?}", self.header.entries);

        if self.header.num_images > 1 {
            warn!("Unstable feature; parsing cursor with more than one stored image");
        }

        let mut dibs = Vec::with_capacity(self.header.entries.len());

        for entry in &self.header.entries {
            let offset = entry.image_offset as usize;
            let size = entry.image_size as usize;
            let dib_blob_range = offset..(offset + size);
            let dib_blob = &self.blob[dib_blob_range];
            let header = BitmapInfoHeader::read(&mut Cursor::new(&dib_blob)).into_diagnostic()?;

            let dib = DeviceIndependentBitmap {
                blob: dib_blob.to_vec(),
                header,
            };

            dibs.push(dib);
        }

        Ok(dibs)
    }
}

/// Models the byte layout of a `BITMAPINFOHEADER`. This is needed
/// for parsing `.bmp` files in memory, used in the `.cur` format.
///
/// More specifically, this is for DIBs. (device independent bitmaps).
///
/// References:
///
/// - <https://en.wikipedia.org/wiki/BMP_file_format#DIB_header_(bitmap_information_header)>
/// - <https://learn.microsoft.com/en-us/previous-versions/ms969901(v=msdn.10)#the-dib-header>
#[derive(BinRead, Debug)]
#[br(
    little,
    assert(header_size == 40, "`BITMAPINFOHEADER` size must be 40."),
    assert(_color_planes == 1, "`color_planes` is a reserved field and must be 1."),
    assert(_height != 0, "Bitmap height cannot be zero."),
    assert(width != 0, "Bitmap width cannot be zero."),
    // ^ The `.bmp` format supports other depths, but
    //   these are the only depths supported for `.cur`.
)]
pub(super) struct BitmapInfoHeader {
    /// Size of the header itself in bytes.
    pub header_size: u32,
    /// (signed) Bitmap width in pixels.
    pub width: i32,
    /// (signed) Bitmap height in pixels.
    ///
    /// NOTE: Use the [`Self::height`] function.
    _height: i32,
    /// Number of color planes; must be 1
    _color_planes: u16,
    /// Also known as color depth; must be 1, 4, 8, or 24
    pub bits_per_pixel: BitsPerPixel,
    /// Type of compression being used on the image.
    pub compression_method: CompressionMethod,
    /// Image size in bytes.
    ///
    /// NOTE: Use the [`Self::image_size`] function
    _image_size: u32,

    /// Default calculated image size in bytes.
    ///
    /// NOTE: Use the [`Self::image_size`] function.
    #[br(
        calc = (((((width * bits_per_pixel as i32) + 31) & !31) >> 3) * _height.abs())
        .try_into().unwrap())
    ]
    _image_size_default: u32,

    /// (signed) Horizontal resolution of image in pixels per metre.
    _horizontal_ppm: i32,
    /// (signed) Vertical resolution of image in pixels per metre.
    _vertical_ppm: i32,
    /// Number of colors in the color palette.
    ///
    /// NOTE: use the [`Self::color_count`] function.
    _color_count: u32,

    /// Default color count.
    ///
    /// NOTE: use the [`Self::color_count`] function.
    #[br(calc = 2u32.pow(bits_per_pixel as u32))]
    _color_count_default: u32,

    /// Number of "important" colors used; generally useless.
    _imp_color_count: u32,
}

impl BitmapInfoHeader {
    /// Returns the canonical image height, which
    /// is half of the `height` field.
    ///
    /// This is because it includes the height of
    /// both the XOR mask and the AND mask, and
    /// since each pixel has their own masks, this
    /// ends up being double the actual image's height.
    pub(super) fn height(&self) -> i32 {
        self._height / 2
    }

    /// Returns the canonical image size.
    ///
    /// Note that this **doesn't** use the [`Self::image_size`]
    /// field because it's **unreliable** since some authors
    /// choose to include the AND mask's size, some don't.
    pub(super) fn image_size(&self) -> u32 {
        // This is divided by two since the height is doubled.
        // Refer to the [`Self::height`] function's documentation.
        self._image_size_default / 2
    }

    /// Returns the canonical color count.
    pub(super) fn color_count(&self) -> u32 {
        if self._color_count == 0 {
            self._color_count_default
        } else {
            self._color_count
        }
    }
}

/// A field in `BITMAPINFOHEADER`, specifying bit depth.
///
/// Reference: <https://en.wikipedia.org/wiki/BMP_file_format#DIB_header_(bitmap_information_header)>
#[derive(BinRead, Debug, PartialEq, Clone, Copy)]
#[br(little, repr = u16)]
#[allow(missing_docs)]
pub(super) enum BitsPerPixel {
    One = 1,
    Four = 4,
    Eight = 8,
    TwentyFour = 24,
}

/// A field in `BITMAPINFOHEADER` used to specify
/// the compression method used for its image.
///
/// Reference: <https://en.wikipedia.org/wiki/BMP_file_format#DIB_header_(bitmap_information_header)>
///
/// ( _find the "Compression method" table!_ )
#[derive(BinRead, Debug, PartialEq)]
#[br(little, repr = u32)]
#[allow(missing_docs)]
pub(super) enum CompressionMethod {
    /// This is the only supported compression method.
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
