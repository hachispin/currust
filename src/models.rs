//! Contains structs modeling byte layouts of Windows cursors.
//!
//! Note that the `.cur` format follows little-endian byte ordering.

#![allow(unused)] // TODO: REMOVE THIS LATER PLEASE

use std::{fs, io::Cursor, path::Path};

use binrw::BinRead;
use log::{ParseLevelError, debug, warn};
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

/// stupd aggergaete
#[derive(Debug)]
struct DeviceIndependentBitmap {
    blob: Vec<u8>,
    header: BitmapInfoHeader,
}

/// cursor
#[derive(Debug)]
pub struct WinCursor {
    blob: Vec<u8>,
    header: IconDir,
}

impl WinCursor {
    pub fn new(cur: &Path) -> Result<Self> {
        let bytes = fs::read(cur).into_diagnostic()?;
        let header = IconDir::read(&mut Cursor::new(&bytes)).into_diagnostic()?;

        Ok(Self {
            blob: bytes,
            header,
        })
    }

    fn extract_dibs(&self) -> Result<Vec<DeviceIndependentBitmap>> {
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
/// - <https://learn.microsoft.com/en-us/previous-versions/ms969901(v=msdn.10)>
///
/// ( _find the "BITMAPINFOHEADER" tables!_ )
#[derive(BinRead, Debug)]
#[br(
    little,
    assert(header_size == 40),
    assert(color_planes == 1),
    assert(width != 0),
    assert(height != 0),
    assert([1, 4, 8, 24].contains(&bits_per_pixel))
)]
pub struct BitmapInfoHeader {
    /// size of the header itself in bytes
    header_size: u32,
    /// (signed) bitmap width in pixels
    pub width: i32,
    /// (signed) bitmap height in pixels
    pub height: i32,
    /// number of color planes (must be 1)
    color_planes: u16,
    /// the color depth; must be 1, 4, 8, and 24
    pub bits_per_pixel: u16,
    /// type of compression being used on image
    compression_method: CompressionMethod,
    /// size of the DIBs pixel array (i.e, XOR mask)
    ///
    /// NOTE: use the [`Self::image_size`] function
    image_size: u32,

    /// default calculated size, explanation can be found here:
    ///
    /// <https://learn.microsoft.com/en-us/previous-versions/ms969901(v=msdn.10)#overview>
    ///
    /// NOTE: use the [`Self::image_size`] function
    #[br(
        calc = (((((width * bits_per_pixel as i32) + 31) & !31) >> 3) * height)
        .try_into().unwrap())
    ]
    image_size_default: u32,

    /// (signed) horizontal resolution of image (pixel per metre)
    _horizontal_ppm: i32,
    /// (signed) vertical resolution of image (pixel per metre)
    _vertical_ppm: i32,
    /// number of colors in color palette
    ///
    /// NOTE: use the [`Self::color_count`] function
    color_count: u32,

    /// default color count
    ///
    /// reference: <https://en.wikipedia.org/wiki/BMP_file_format#Color_table>
    ///
    /// NOTE: use the [`Self::color_count`] function
    #[br(calc = 2u32.pow(bits_per_pixel as u32))]
    color_count_default: u32,

    /// number of "important" colors used; generally useless
    _imp_color_count: u32,
}

impl BitmapInfoHeader {
    /// Returns the canonical image size.
    fn image_size(&self) -> u32 {
        if self.image_size == 0 {
            self.image_size_default
        } else {
            self.image_size
        }
    }

    /// Returns the canonical color count.
    fn color_count(&self) -> u32 {
        if self.color_count == 0 {
            self.color_count_default
        } else {
            self.color_count
        }
    }
}

/// A field in `BITMAPINFOHEADER` used to specify
/// the compression method used for its image.
///
/// Reference: <https://en.wikipedia.org/wiki/BMP_file_format#DIB_header_(bitmap_information_header)>
///
/// ( _find the "Compression method" table!_ )
#[derive(BinRead, Debug, PartialEq)]
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

#[derive(Debug)]
pub struct CursorImage {
    pub rgba: Vec<u8>,
    hotspot_x: i32,
    hotspot_y: i32,
    pub width: u32,
    pub height: u32,
}

impl CursorImage {
    pub fn from_cur(cur: WinCursor) -> Result<Vec<CursorImage>> {
        let dibs = cur.extract_dibs()?;
        let mut images = Vec::with_capacity(dibs.len());

        // surely order is guaranteed... please
        for (entry, dib) in cur.header.entries.iter().zip(dibs) {
            let rgba = Self::extract_rgba(&dib);

            if dib.header.width != entry.width as i32 {
                warn!(
                    "Mismatched widths: dib.header.width={}, entry.width={}",
                    dib.header.width, entry.width
                );
            }

            let image = CursorImage {
                rgba,
                hotspot_x: entry.hotspot_x as i32,
                hotspot_y: entry.hotspot_y as i32,
                width: dib.header.width as u32,
                height: (dib.header.height / 2) as u32,
            };

            images.push(image);
        }

        Ok(images)
    }

    fn get_alpha_bits(alpha: &[u8]) -> Vec<bool> {
        let mut alpha_bits = Vec::with_capacity(alpha.len() * 8);

        for byte in alpha {
            for i in 0..8 {
                let bit = (byte & (1 << (7 - i)));
                alpha_bits.push(bit != 0);
            }
        }

        alpha_bits
    }

    /// Extracts and returns a raw RGBA blob from the provided `dib`.
    fn extract_rgba(dib: &DeviceIndependentBitmap) -> Vec<u8> {
        assert_eq!(
            dib.header.compression_method,
            CompressionMethod::RGB,
            "compression methods other than rgb (uncompressed) not implemented"
        );

        assert_eq!(
            dib.header.bits_per_pixel, 8,
            "bits per pixel values other than 8 not implemented"
        );

        assert!(
            dib.header.bits_per_pixel == 8,
            "wrong rgba parse method used"
        );
        assert!(dib.header.width.is_positive()); // negative width is undefined
        assert!(dib.header.color_count() != 0); // palette is required for bpp <= 8

        let mut rgba = Vec::with_capacity(dib.blob.len());

        // generally, from left to right, the order is:
        //
        // ╭──────────┬───────────┬──────────┬──────────╮
        // │  HEADER  │  PALETTE  │ XOR MASK │ AND MASK │
        // ╰──────────┴───────────┴──────────┴──────────╯
        //
        // The XOR mask is where pixel data is stored.
        // The AND mask is where alpha data is stored
        // (note: only fully opaque and fully transparent are supported)

        let header_size = dib.header.header_size as usize;
        let width = dib.header.width as usize;
        let height = dib.header.height as usize / 2;

        let palette_offset = header_size;
        let pixel_data_offset = header_size + dib.header.color_count() as usize * 4;

        let row_size_unpadded_bits = dib.header.bits_per_pixel as usize * dib.header.width as usize;
        let row_size_unpadded = row_size_unpadded_bits / 8;
        let row_size = row_size_unpadded_bits.div_ceil(32) * 4; // 4-byte alignment

        let alpha_offset = pixel_data_offset + (row_size * height);
        let alpha_size = height * width.div_ceil(32) * 4; // 4-byte alignment for bits
        let alpha_bits = Self::get_alpha_bits(&dib.blob[alpha_offset..(alpha_offset + alpha_size)]);

        // reverse if positive, normal if negative
        let row_indices: Vec<usize> = if dib.header.height.is_positive() {
            (0..height).rev().collect()
        } else {
            (0..height).collect()
        };

        for row_index in row_indices {
            let row_offset = row_size * row_index;
            let row_start = pixel_data_offset + row_offset;
            let row = &dib.blob[row_start..(row_start + row_size_unpadded)];

            for (i, color_index) in row.into_iter().map(|i| *i as usize).enumerate() {
                // The row, which contains palette indices:
                // [0, 1, 2, ...]
                //  │  │  ╰─────────────────╮
                //  │  ╰────────╮           │
                // [B, G, R, _, B, G, R, _, B, G, R, _, ...]
                // ^ The palette, contiguous array of pixels
                //
                // Therefore, we need to multiply the indices by four.
                // (as an aside, Windows stores pixels as BGR, not RGB.)
                // (... as another aside, the 4th byte in pixels are reserved)
                // (... seriously, what is going on?)

                let palette_index = color_index * 4;
                let pixel_start = palette_offset + palette_index;
                let pixel = &dib.blob[pixel_start..(pixel_start + 3)];

                rgba.extend(pixel.into_iter().rev());

                // get position of current pixel
                let alpha_index = row_index * dib.header.width as usize + i;

                if alpha_bits[alpha_index] {
                    rgba.push(0);
                } else {
                    rgba.push(255);
                }
            }
        }

        rgba
    }
}
