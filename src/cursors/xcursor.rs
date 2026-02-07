//! Module used to write Xcursor from [`GenericCursor`].
//!
//! The Xcursor format is stored as such:
//!
//! 1) Magic bytes "Xcur", indicating it's, well, Xcursor.
//! 2) The [`XcursorHeader`], which contains the required [`TableOfContents`].
//!    Each of these entries point to either an image or comment chunk.
//! 3) The [`ImageChunk`] s, which store **big-endian ARGB** pixel data,
//!    along with other required metadata such as the hotspot.
//!
//! NOTE: Comment chunks aren't handled to make code
//!       simpler, we're only _writing_, after all.
//!
//! ## References
//!
//! - [Xcursor](https://manpages.ubuntu.com/manpages/plucky/man3/Xcursor.3.html)
//! - [libXcursor source](https://gitlab.freedesktop.org/xorg/lib/libxcursor)
//! - [xcursorgen](https://gitlab.freedesktop.com/xorg/app/xcursorgen)
// god i don't wanna write a parser again

use super::{cursor_image::CursorImage, generic_cursor::GenericCursor};

use anyhow::Result;
use binrw::binwrite;

/* Sizes are needed for certain fields, which is also why they're u32.
 * An extra benefit is that they help in pointer calculations, since
 * binrw deletes calculated fields in structs, so size_of doesn't work. */

/// Current Xcursor version. May need updating in the future.
const XCURSOR_HEADER_VERSION: u32 = 1 << 16;

/// Size of [`XcursorHeader`], including elided fields and its magic, excluding `toc`.
const XCURSOR_HEADER_SIZE: u32 = 16;

/// Size of [`TableOfContents`], including elided fields.
const TOC_SIZE: u32 = 12;

/// Size of [`ImageChunk`], excluding `argb`.
const IMAGE_HEADER_SIZE: u32 = 36;

/// Magic value used to indicate a chunk is an [`ImageChunk`].
const IMAGE_TYPE: u32 = 0xfffd_0002;

/// Current version stored in [`ImageChunk`].
const IMAGE_VERSION: u32 = 1;

/// Represents the file header for Xcursor files.
#[binwrite]
#[derive(Debug)]
#[bw(little, magic = b"Xcur")]
struct XcursorHeader {
    #[bw(calc = XCURSOR_HEADER_SIZE)]
    header_size: u32,
    #[bw(calc = XCURSOR_HEADER_VERSION)]
    version: u32,
    #[bw(try_calc = toc.len().try_into())]
    num_toc: u32,

    /// TOC entries which lead to [`ImageChunk`] s.
    toc: Vec<TableOfContents>,
}

/// A table of contents that stores metadata regarding a chunk.
///
/// This should always lead to an [`ImageChunk`].
#[binwrite]
#[derive(Debug, Clone)]
#[bw(little)]
struct TableOfContents {
    #[bw(calc = IMAGE_TYPE)]
    r#type: u32,
    /// The (image) chunk's nominal size, for matching.
    ///
    /// For example, for a 32x32 cursor, the nominal size
    /// would most likely be 32. If dimensions aren't equal,
    /// the largest dimension is chosen. (e.g, 48 for 48x32).
    nominal_size: u32,
    /// Offset where this entry's chunk is found.
    ///
    /// This should be where image data starts.
    position: u32,
}

/// Stores an image as [`Self::argb`], along with
/// some additional metadata needed for cursors.
///
/// This also stores its own header, since there's no need
/// to de-duplicate common fields with only one chunk type.
#[binwrite]
#[derive(Debug)]
#[bw(little)]
struct ImageChunk {
    #[bw(calc = IMAGE_HEADER_SIZE)]
    header_size: u32,
    #[bw(calc = IMAGE_TYPE)]
    chunk_type: u32,
    #[bw(calc = *width.max(height))]
    nominal_size: u32,
    #[bw(calc = IMAGE_VERSION)]
    version: u32,

    // original libxcursor checks for (w, h <= 32,767) but constrained it further
    // https://gitlab.freedesktop.org/xorg/lib/libxcursor/-/blob/master/include/X11/Xcursor/Xcursor.h.in?ref_type=heads#L183
    #[bw(assert(*width != 0, *width <= 2048))]
    width: u32,
    #[bw(assert(*height != 0, *height <= 2048))]
    height: u32,
    #[bw(assert(hotspot_x <= width))]
    hotspot_x: u32,
    #[bw(assert(hotspot_y <= height))]
    hotspot_y: u32,

    /// The time (in milliseconds) that this
    /// frame is displayed for before the next.
    delay: u32,

    /// Pre-multiplied big-endian ARGB image data.
    // despite this being big-endian, we can write
    // in little endian with bgra quads equivalently
    // so, NOTE: don't add #[bw(big)] to this
    #[bw(assert(argb.len() == usize::try_from(width * height).unwrap()))]
    argb: Vec<u32>,
}

impl From<&CursorImage> for ImageChunk {
    fn from(image: &CursorImage) -> Self {
        let mut rgba = image.rgba().to_owned();
        to_pre_argb(&mut rgba);

        let argb = to_u32_vec(&rgba);
        let (width, height) = image.dimensions();
        let (hotspot_x, hotspot_y) = image.hotspot();
        let delay = image.delay();

        Self {
            width,
            height,
            hotspot_x,
            hotspot_y,
            delay,
            argb,
        }
    }
}

/// Models the Xcursor format.
///
/// This should produce a valid file when written.
#[binwrite]
#[derive(Debug)]
#[bw(little)]
pub struct Xcursor {
    #[bw(assert(header.toc.len() == images.len()))]
    header: XcursorHeader,
    images: Vec<ImageChunk>,
}

/// Converts RGBA packed pixels to pre-multiplied big-endian ARGB in-place.
///
/// If `rgba.len()` is not a multiple of four, the remainder is discarded.
fn to_pre_argb(rgba: &mut [u8]) {
    for pixel in rgba.as_chunks_mut::<4>().0 {
        // write as LE-BGRA which is equivalent to BE-ARGB
        // less swaps needed and NE speeds (if on LE arch)
        pixel.swap(0, 2);

        for i in 0..3usize {
            pixel[i] = pre_alpha_formula(pixel[i], pixel[3]);
        }
    }
}

/// Converts u8s to u32s using little-endian.
///
/// If `u8_vec.len()` is not a multiple of four, the remainder is discarded.
fn to_u32_vec(u8_vec: &[u8]) -> Vec<u32> {
    u8_vec
        .as_chunks::<4>()
        .0
        .iter()
        .map(|q| u32::from_le_bytes(*q))
        .collect()
}

/// Formula used for pre-multiplying a color channel with an alpha channel.
#[allow(clippy::cast_possible_truncation)]
const fn pre_alpha_formula(c: u8, a: u8) -> u8 {
    // +127 rounds to closest integer instead of floor
    let prod = (c as u16) * (a as u16);
    ((prod + 127) / 255) as u8
}

impl Xcursor {
    /// Converts `cursor` to Xcursor format.
    ///
    /// ## Errors
    ///
    /// If [`TryInto`] conversions fail between primitive
    /// types. No other error can occur other than this.
    pub fn new(cursor: &GenericCursor) -> Result<Self> {
        let num_toc = cursor.num_images();
        let num_toc_u32 = u32::try_from(num_toc)?;
        let toc_offset = XCURSOR_HEADER_SIZE;
        let image_offset = toc_offset + (num_toc_u32 * TOC_SIZE);

        let mut toc = Vec::with_capacity(num_toc);
        let mut images = Vec::with_capacity(num_toc);
        let mut position = image_offset;

        for image in cursor.joined_images() {
            let nominal_size = image.nominal_size();
            let image_chunk_size = IMAGE_HEADER_SIZE + u32::try_from(image.rgba().len())?;

            let toc_entry = TableOfContents {
                nominal_size,
                position,
            };

            debug_assert_eq!(toc_entry.nominal_size, image.nominal_size());
            toc.push(toc_entry);
            images.push(ImageChunk::from(image));

            position += image_chunk_size;
        }

        Ok(Self {
            header: XcursorHeader { toc },
            images,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::cursors::cursor_image::{
        CursorImages,
        test::{BLACK, WHITE},
    };

    use binrw::BinWrite;
    use std::{io::BufWriter, ptr::NonNull};

    macro_rules! denullify {
        ($ptr:expr, $($msg:tt)*) => {
            NonNull::new($ptr).unwrap_or_else(|| panic!($($msg)*))
        };
    }

    /// Generates an Xcursor with 10 alternating black and white
    /// frames, with a delay of 100ms between each frame.
    fn black_and_white() -> Xcursor {
        // 32x32 = 1024, 1024 * 4 = 4096, so each frame is 4096 bytes (u8s).
        let mut frames: Vec<CursorImage> = Vec::new();

        for i in 0..9 {
            if i % 2 == 0 {
                frames.push(BLACK.clone());
            } else {
                frames.push(WHITE.clone());
            }
        }

        let frames = CursorImages::try_from(frames).unwrap();
        let cursor = GenericCursor::new_unscaled(frames);
        Xcursor::new(&cursor).unwrap()
    }

    #[cfg(target_os = "linux")] // libXcursor is dynamically-linked
    #[test]
    /// Attempts to load the cursor produced from `black_and_white()` with libXcursor.
    fn libxcursor() {
        use libc::{SEEK_SET, fdopen, lseek};
        use std::os::fd::AsRawFd;
        use tempfile::tempfile;
        use x11::xcursor::XcursorFileLoadImages;

        let file = tempfile().unwrap();
        let xcursor = self::black_and_white();
        let raw_fd = file.as_raw_fd();

        xcursor.write(&mut BufWriter::new(&file)).unwrap();

        if unsafe { lseek(raw_fd, 0, SEEK_SET) } == -1 {
            panic!("lseek() returned -1 with raw_fd={raw_fd}, offset=0, whence=SEEK_SET")
        }

        let c_file = denullify!(
            unsafe { fdopen(raw_fd, c"r".as_ptr()) },
            "fdopen() returned NULL with raw_fd={raw_fd}"
        );

        let _image_ptr = denullify!(
            unsafe { XcursorFileLoadImages(c_file.as_ptr(), 32) },
            "XcursorFileLoadImages() returned NULL with raw_fd={raw_fd}, c_file={:p}",
            c_file.as_ptr()
        );

        // no fclose() needed, fs::File manages it
    }
}
