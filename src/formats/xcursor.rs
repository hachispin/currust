//! Module used to write Xcursor from [`GenericCursor`].
//!
//! The Xcursor format is stored as such:
//!
//! 1) Magic bytes "Xcur", indicating it's, well, Xcursor.
//! 2) The `XcursorHeader`, which contains the required `TableOfContents`.
//!    Each of these entries point to either an image or comment chunk.
//! 3) The `ImageChunk` s, which store **big-endian ARGB** pixel data,
//!    along with other required metadata such as the hotspot.
//!
//! Comment chunks may be written as well.
//!
//! ## References
//!
//! - [Xcursor](https://manpages.ubuntu.com/manpages/plucky/man3/Xcursor.3.html)
//! - [libXcursor source](https://gitlab.freedesktop.org/xorg/lib/libxcursor)
//! - [xcursorgen](https://gitlab.freedesktop.com/xorg/app/xcursorgen)

use crate::cursors::{cursor_image::CursorImage, generic_cursor::GenericCursor};

use anyhow::Result;
use binrw::binwrite;
use bytemuck;

/// Versions numbers. May be subject to change.
mod versions {
    pub const XCURSOR: u32 = 1 << 16;
    pub const COMMENT: u32 = 1;
    pub const IMAGE: u32 = 1;
}

/// Sizes (of fixed-size fields) for position calculations.
mod sizes {
    pub const XCURSOR: u32 = 16;
    pub const COMMENT: u32 = 20;
    pub const IMAGE: u32 = 36;
    pub const TOC: u32 = 12;
}

#[binwrite]
#[bw(repr = u32)]
#[derive(Debug, Clone, Copy)]
#[repr(u32)]
enum ChunkType {
    Comment = 0xfffe_0001,
    Image = 0xfffd_0002,
}

#[binwrite]
#[bw(repr = u32)]
#[derive(Debug, Clone, Copy)]
#[repr(u32)]
enum CommentRole {
    Copyright = 1,
    License = 2,
    Other = 3,
}

/// Represents the file header for Xcursor files.
#[binwrite]
#[bw(little, magic = b"Xcur")]
#[derive(Debug)]
struct XcursorHeader {
    #[bw(calc = sizes::XCURSOR)]
    header_size: u32,
    #[bw(calc = versions::XCURSOR)]
    version: u32,
    #[bw(try_calc = toc.len().try_into())]
    num_toc: u32,

    /// TOC entries which lead to comment/image chunks.
    toc: Vec<TableOfContents>,
}

/// A table of contents that stores metadata regarding a chunk.
///
/// This should always lead to an [`ImageChunk`].
#[binwrite]
#[bw(little)]
#[derive(Debug, Clone)]
struct TableOfContents {
    r#type: ChunkType,
    /// Can be either the nominal size for
    /// images or [`CommentRole`] for comments.
    subtype: u32,
    position: u32,
}

#[binwrite]
#[bw(little)]
#[derive(Debug)]
struct CommentChunk {
    #[bw(calc = sizes::COMMENT)]
    header_size: u32,
    #[bw(calc = ChunkType::Comment)]
    r#type: ChunkType,

    role: CommentRole,

    #[bw(calc = versions::COMMENT)]
    version: u32,
    #[bw(try_calc = string.len().try_into())]
    length: u32,

    /// The comment to be stored.
    string: Vec<u8>,
}

impl CommentChunk {
    fn new(string: String, subtype: CommentRole, position: u32) -> (Self, TableOfContents) {
        let comment = Self {
            role: subtype,
            string: string.into_bytes(),
        };

        let toc = TableOfContents {
            r#type: ChunkType::Comment,
            subtype: comment.role as u32,
            position,
        };

        (comment, toc)
    }
}

/// Stores an image as [`Self::argb`], along with
/// some additional metadata needed for cursors.
#[binwrite]
#[bw(little)]
#[derive(Debug)]
struct ImageChunk {
    #[bw(calc = sizes::IMAGE)]
    header_size: u32,
    #[bw(calc = ChunkType::Image)]
    chunk_type: ChunkType,
    nominal_size: u32,
    #[bw(calc = versions::IMAGE)]
    version: u32,

    #[bw(assert(*width != 0, *width <= 2048))]
    width: u32,
    #[bw(assert(*height != 0, *height <= 2048))]
    height: u32,
    #[bw(assert(hotspot_x <= width))]
    hotspot_x: u32,
    #[bw(assert(hotspot_y <= height))]
    hotspot_y: u32,
    /// Uses milliseconds.
    #[bw(assert(*delay <= 60_000))]
    delay: u32,

    /// Pre-multiplied big-endian ARGB image data.
    // NOTE: Don't add #[bw(big)] to this.
    #[bw(assert(argb.len() == usize::try_from(width * height).unwrap()))]
    argb: Vec<u32>,
}

impl ImageChunk {
    fn new(image: &CursorImage, position: u32) -> (Self, TableOfContents) {
        let toc = TableOfContents {
            r#type: ChunkType::Image,
            // nominal size
            subtype: image.nominal_size(),
            position,
        };

        let image = Self::from(image);

        (image, toc)
    }
}

impl From<&CursorImage> for ImageChunk {
    fn from(image: &CursorImage) -> Self {
        let nominal_size = image.nominal_size();
        let (width, height) = image.dimensions();
        let (hotspot_x, hotspot_y) = image.hotspot();
        let delay = image.delay();

        let mut rgba = image.rgba().to_owned();
        to_pre_argb(&mut rgba);
        let argb = bytemuck::pod_collect_to_vec(&rgba);

        Self {
            nominal_size,
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
#[bw(little)]
#[derive(Debug)]
pub struct Xcursor {
    #[bw(assert(header.toc.len() == images.len() + comment.as_ref().map_or(0, |_| 1)))]
    header: XcursorHeader,
    comment: Option<CommentChunk>,
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

/// Formula used for pre-multiplying a color channel with an alpha channel.
#[allow(clippy::cast_possible_truncation, clippy::inline_always)]
#[inline(always)]
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
        let num_toc = cursor.num_images() + cursor.info().map_or(0, |_| 1);
        let num_toc_u32 = u32::try_from(num_toc)?;
        let chunks_offset = sizes::XCURSOR + (num_toc_u32 * sizes::TOC);

        let mut toc = Vec::with_capacity(num_toc);
        let mut images = Vec::with_capacity(num_toc);
        let mut position = chunks_offset;

        let comment = if let Some(info) = cursor.info() {
            let info_len = u32::try_from(info.len())?;
            let (chunk, toc_entry) = CommentChunk::new(info, CommentRole::Other, position);
            position += sizes::COMMENT + info_len;
            toc.push(toc_entry);
            Some(chunk)
        } else {
            None
        };

        for image in cursor.joined_images() {
            let image_chunk_size = sizes::IMAGE + u32::try_from(image.rgba().len())?;
            let (chunk, toc_entry) = ImageChunk::new(image, position);

            toc.push(toc_entry);
            images.push(chunk);

            position += image_chunk_size;
        }

        Ok(Self {
            header: XcursorHeader { toc },
            comment,
            images,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::cursors::generic_cursor::test::black_and_white;

    use std::{
        io::{BufWriter, Seek, SeekFrom},
        ptr::NonNull,
    };

    use binrw::BinWrite;
    use tempfile::tempfile;

    macro_rules! denullify {
        ($ptr:expr, $($msg:tt)*) => {
            NonNull::new($ptr).unwrap_or_else(|| panic!($($msg)*))
        };
    }

    #[cfg(target_os = "linux")] // libXcursor is dynamically-linked
    #[test]
    /// Attempts to load the cursor produced from `black_and_white()` with libXcursor.
    fn libxcursor() {
        use libc::fdopen;
        use std::os::fd::AsRawFd;
        use x11::xcursor::XcursorFileLoadImages;

        let mut file = tempfile().unwrap();
        let xcursor = Xcursor::new(&black_and_white()).unwrap();
        let raw_fd = file.as_raw_fd();

        xcursor.write(&mut BufWriter::new(&file)).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();

        let c_file = denullify!(
            unsafe { fdopen(raw_fd, c"r".as_ptr()) },
            "fdopen() returned NULL with raw_fd={raw_fd}"
        );

        // if this is not null, that's a pass
        let _image_ptr = denullify!(
            unsafe { XcursorFileLoadImages(c_file.as_ptr(), 32) },
            "XcursorFileLoadImages() returned NULL with raw_fd={raw_fd}, c_file={:p}",
            c_file.as_ptr()
        );

        // no fclose() needed, fs::File manages it
    }
}
