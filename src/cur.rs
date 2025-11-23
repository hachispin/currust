//! Contains structs modeling byte layouts of Windows cursors.
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
pub struct IconDir {
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
pub struct IconDirEntry {
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
    image_size: u32,
    /// Offset of image data from the beginning of `.cur` blob.
    ///
    /// For `.bmp`, this leads you to `BITMAPINFOHEADER`.
    pub image_offset: u32,
}

/// Aggregate for DIB structure.
#[derive(Debug)]
struct DeviceIndependentBitmap {
    blob: Vec<u8>,
    header: BitmapInfoHeader,
}

/// Full representation of a Windows cursor.
#[derive(Debug)]
pub struct WinCursor {
    blob: Vec<u8>,
    header: IconDir,
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
    fn extract_dibs(&self) -> Result<Vec<DeviceIndependentBitmap>> {
        debug!("Extracting DIBs from entries={:?}", self.header.entries);
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
    assert([1, 4, 8, 24].contains(&bits_per_pixel),
        "Invalid bit depth, bits_per_pixel={bits_per_pixel}")
    // ^ The `.bmp` format supports other depths, but
    //   these are the only depths supported for `.cur`.
)]
pub struct BitmapInfoHeader {
    /// Size of the header itself in bytes.
    header_size: u32,
    /// (signed) Bitmap width in pixels.
    width: i32,
    /// (signed) Bitmap height in pixels.
    ///
    /// NOTE: Use the [`Self::height`] function.
    _height: i32,
    /// Number of color planes; must be 1
    _color_planes: u16,
    /// Color depth; must be 1, 4, 8, or 24
    bits_per_pixel: u16,
    /// Type of compression being used on the image.
    compression_method: CompressionMethod,
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

    /// default color count
    ///
    /// NOTE: use the [`Self::color_count`] function
    #[br(calc = 2u32.pow(bits_per_pixel as u32))]
    _color_count_default: u32,

    /// number of "important" colors used; generally useless
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
    fn height(&self) -> i32 {
        self._height / 2
    }

    /// Returns the canonical image size.
    ///
    /// Note that this **doesn't** use the [`Self::image_size`]
    /// field because it's **unreliable** since some authors
    /// choose to include the AND mask's size, some don't.
    fn image_size(&self) -> u32 {
        // This is divided by two since the height is doubled.
        // Refer to the [`Self::height`] function's documentation.
        self._image_size_default / 2
    }

    /// Returns the canonical color count.
    fn color_count(&self) -> u32 {
        if self._color_count == 0 {
            self._color_count_default
        } else {
            self._color_count
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

/// Represents a generic cursor.
#[derive(Debug)]
pub struct CursorImage {
    /// Raw image data.
    pub rgba: Vec<u8>,
    /// X coordinates of click point.
    pub hotspot_x: i32,
    /// Y coordinates of click point.
    pub hotspot_y: i32,
    /// Width of the stored image in [`Self::rgba`]
    pub width: u32,
    /// Weight of the stored image in [`Self::rgba`]
    pub height: u32,
}

impl CursorImage {
    /// Converts `cur` to a vector of [`CursorImage`] structs.
    pub fn from_win_cur(cur: WinCursor) -> Result<Vec<CursorImage>> {
        let dibs = cur.extract_dibs()?;
        let mut images = Vec::with_capacity(dibs.len());

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
                height: dib.header.height().abs() as u32,
            };

            images.push(image);
        }

        Ok(images)
    }

    /// Helper function for [`Self::extract_rgba`].
    ///
    /// Splits all the given bytes in `alpha` into bits,
    /// collecting them all (flattened) as [`Vec<bool>`],
    ///
    /// - `0` or `false` means it's fully opaque.
    /// - `1` or `true` means it's fully transparent.
    fn get_alpha_bits(alpha: &[u8]) -> Vec<bool> {
        let mut alpha_bits = Vec::with_capacity(alpha.len() * 8);

        for byte in alpha {
            for i in 0..8 {
                let bit = byte & (1 << (7 - i));
                alpha_bits.push(bit != 0);
            }
        }

        alpha_bits
    }

    /// Extracts and returns a raw RGBA blob from the provided `dib`.
    ///
    /// Note that only [`CompressionMethod::RGB`] is supported.
    fn extract_rgba(dib: &DeviceIndependentBitmap) -> Vec<u8> {
        debug!("Extracting RGBA from DIB, header={:?}", dib.header);

        assert_eq!(
            dib.header.compression_method,
            CompressionMethod::RGB,
            "{:?} not supported",
            dib.header.compression_method,
        );

        // Behaviour for negative width is undefined
        assert!(
            dib.header.width.is_positive(),
            "expected positive width, instead got dib.header.width={}",
            dib.header.width
        );

        // Palette is required for bpp <= 8
        assert!(
            dib.header.color_count() != 0,
            "Missing palette; color count is zero"
        );

        if dib.header.bits_per_pixel != 8 {
            warn!(
                "Unstable feature; extracting RGBA from bits_per_pixel={}",
                dib.header.bits_per_pixel
            );
        }

        if dib.header.height().is_negative() {
            warn!(
                "Unstable feature; extracting RGBA from DIBs with height={}",
                dib.header.height()
            );
        }

        let mut rgba = Vec::with_capacity(dib.header.image_size() as usize * 4);

        // Generally, from left to right, the order is:
        //
        // ╭──────────┬───────────┬────────────┬────────────╮
        // │  HEADER  │  PALETTE  │  XOR MASK  │  AND MASK  │
        // ╰──────────┴───────────┴────────────┴────────────╯
        //
        // The XOR mask is where pixel data is stored.
        // The AND mask is where alpha data is stored.
        //
        // Further reading:
        // https://en.wikipedia.org/wiki/ICO_(file_format)#File_structure

        // Alias some variables for brevity
        let header_size = dib.header.header_size as usize;
        let width = dib.header.width.abs() as usize;
        let height = dib.header.height().abs() as usize;
        let image_size = dib.header.image_size() as usize;
        let color_count = dib.header.color_count() as usize;

        let palette_offset = header_size;
        let pixel_data_offset = header_size + color_count * 4;
        let alpha_offset = pixel_data_offset + image_size;

        debug!(
            "Calculated offsets: palette_offset={}, pixel_data_offset={}, alpha_offset={}, dib.blob.len={}",
            palette_offset,
            pixel_data_offset,
            alpha_offset,
            dib.blob.len()
        );

        // Each row must be a multiple of 4 bytes
        let row_size_unpadded = (dib.header.bits_per_pixel as usize * width) / 8;
        let row_size = row_size_unpadded.next_multiple_of(4); // 4-byte alignment

        // Same thing applies here; rows must be multiples of 4 bytes
        let alpha_size = image_size / 8; // each byte stores 8 transparency flags        
        let alpha_bytes = &dib.blob[alpha_offset..(alpha_offset + alpha_size)];
        let alpha_bits = Self::get_alpha_bits(alpha_bytes);

        // Start reading rows the from bottom if positive, else, start from the top
        //
        //     Positive:            Negative:
        //                         ┌──┐┌──┐┌──┐
        //  3 ...                1 ┿━━┿┿━━┿┿━━┿▶
        //                         └──┘└──┘└──┘
        //    ┌──┐┌──┐┌──┐         ┌──┐┌──┐┌──┐
        //  2 ┿━━┿┿━━┿┿━━┿▶      2 ┿━━┿┿━━┿┿━━┿▶
        //    └──┘└──┘└──┘         └──┘└──┘└──┘
        //    ┌──┐┌──┐┌──┐
        //  1 ┿━━┿┿━━┿┿━━┿▶      3 ...
        //    └──┘└──┘└──┘
        //
        // Numbers here indicate the reading order; 1 is read first
        let row_indices: Vec<usize> = if dib.header.height().is_positive() {
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

                // Get position of current pixel
                let alpha_index = if dib.header.height().is_positive() {
                    row_index * dib.header.width as usize + i
                } else {
                    // -1 because sizes are one-indexed
                    (image_size - 1) - (row_index * dib.header.width as usize + i)
                };

                if alpha_bits[alpha_index] {
                    rgba.push(0); // transparent
                } else {
                    rgba.push(255); // opaque
                }
            }
        }

        rgba
    }
}
