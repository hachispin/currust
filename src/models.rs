//! Contains any common structs.

use crate::cur::{
    BitmapInfoHeader, BitsPerPixel, CompressionMethod, DeviceIndependentBitmap, WinCursor,
};

use bitvec::prelude::*;
use log::{debug, warn};
use miette::Result;

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

/// Helper struct for [`CursorImage::extract_rgba`].
#[derive(Debug)]
struct Offsets {
    palette: usize,
    pixel_data: usize,
    alpha: usize,
}

impl Offsets {
    /// Calculates offsets from the given `header`.
    fn from_header(header: &BitmapInfoHeader) -> Self {
        let header_size = header.header_size as usize;
        let image_size = header.image_size() as usize;
        let color_count = header.color_count() as usize;

        let palette_offset = header_size;
        let pixel_data_offset = header_size + color_count * 4;
        let alpha_offset = pixel_data_offset + image_size;

        Self {
            palette: palette_offset,
            pixel_data: pixel_data_offset,
            alpha: alpha_offset,
        }
    }
}

impl CursorImage {
    /// Converts `cur` to a vector of [`CursorImage`] structs.
    pub fn from_win_cur(cur: WinCursor) -> Result<Vec<CursorImage>> {
        let dibs = cur.extract_dibs()?;
        let mut images = Vec::with_capacity(dibs.len());

        for (entry, dib) in cur.header.entries.iter().zip(dibs) {
            let rgba = Self::extract_rgba(&dib);

            if dib.header.height() != entry.height as i32 {
                warn!(
                    "Conflicting heights: dib.header.height()={}, entry.height={}",
                    dib.header.height(),
                    entry.height
                )
            }

            if dib.header.width != entry.width as i32 {
                warn!(
                    "Conflicting widths: dib.header.width={}, entry.width={}",
                    dib.header.width, entry.width
                );
            }

            if dib.header.image_size() != entry.image_size {
                warn!(
                    "Conflicting image sizes: dib.header.image_size()={}, entry.image_size={}",
                    dib.header.image_size(),
                    entry.image_size
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
    fn get_palette_indices(byte: u8, bits_per_pixel: u16, palette_offset: usize) -> Vec<usize> {
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

        if bits_per_pixel == 8 {
            let index = byte as usize * 4 + palette_offset;
            return vec![index];
        }

        if bits_per_pixel == 4 {
            let (i, j) = byte.view_bits::<Msb0>().split_at(4);
            let i = i.load::<u8>() as usize * 4 + palette_offset;
            let j = j.load::<u8>() as usize * 4 + palette_offset;

            return vec![i, j];
        }

        if bits_per_pixel == 1 {
            let bits = byte.view_bits::<Msb0>();
            let indices: Vec<usize> = bits
                .iter()
                .map(|b| *b as usize * 4 + palette_offset)
                .collect();
            return indices;
        }

        if bits_per_pixel == 24 {
            todo!()
        }

        unreachable!()
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

        if dib.header.bits_per_pixel == BitsPerPixel::TwentyFour {
            todo!("Extracting RGBA from 24bpp images isn't implemented yet");
        }

        if dib.header.height().is_negative() {
            warn!(
                "Unstable feature; extracting RGBA from DIBs with height={}",
                dib.header.height()
            );
        }

        let rgba_capacity = dib.header.image_size() as f64
            * match dib.header.bits_per_pixel {
                BitsPerPixel::One => 32.0, // 1 bit per pixel => 8 RGBA pixels per byte => 32 RGBA bytes
                BitsPerPixel::Four => 8.0, // 4 bits per pixel => 2 RGBA pixels per byte => 8 RGBA bytes
                BitsPerPixel::Eight => 4.0, // ...
                BitsPerPixel::TwentyFour => 4.0 / 3.0,
            };

        let mut rgba = Vec::with_capacity(rgba_capacity.ceil() as usize);

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
        let width = dib.header.width.abs() as usize;
        let height = dib.header.height().abs() as usize;
        let image_size = dib.header.image_size() as usize;
        let bits_per_pixel = dib.header.bits_per_pixel as u16;

        let offsets = Offsets::from_header(&dib.header);

        debug!(
            "Calculated offsets (dib.blob.len={}): {:?}",
            dib.blob.len(),
            offsets,
        );

        // Each row must be a multiple of 4 bytes
        let row_size_unpadded = (dib.header.bits_per_pixel as usize * width) / 8;
        let row_size = row_size_unpadded.next_multiple_of(4); // 4-byte alignment

        // Same thing applies here; rows must be multiples of 4 bytes
        let pixels_per_byte = (8 / bits_per_pixel) as usize;
        let alpha_size = (image_size / 8) * pixels_per_byte; // each byte stores 8 transparency flags        
        let alpha_bytes = &dib.blob[offsets.alpha..(offsets.alpha + alpha_size)];
        let alpha_bits = alpha_bytes.view_bits::<Msb0>();

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

        dbg!(alpha_bits.len(), image_size);

        for row_index in row_indices {
            let row_offset = row_size * row_index;
            let row_start = offsets.pixel_data + row_offset;
            let row = &dib.blob[row_start..(row_start + row_size_unpadded)];

            for (row_pos, color_byte) in row.into_iter().enumerate() {
                let palette_indices =
                    Self::get_palette_indices(*color_byte, bits_per_pixel, offsets.palette);

                for (byte_pos, palette_index) in palette_indices.into_iter().enumerate() {
                    let pixel = &dib.blob[palette_index..palette_index + 3];
                    rgba.extend(pixel.into_iter().rev());

                    let alpha_index = if dib.header.height().is_positive() {
                        (row_offset + row_pos) * pixels_per_byte + byte_pos
                    } else {
                        // Untested but should work... in theory?
                        alpha_bits.len() - ((row_offset + row_pos) * pixels_per_byte + byte_pos) - 1
                    };

                    let alpha = if alpha_bits[alpha_index] { 0 } else { 255 };

                    rgba.push(alpha);
                }
            }
        }

        rgba
    }
}
