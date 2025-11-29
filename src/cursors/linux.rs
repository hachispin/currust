//! Contains structs that model byte layouts of
//! the [Xcursor](https://man.archlinux.org/man/Xcursor.3) format.
//!
//! The Xcursor format generally follows this structure:
//!
//! 1) File header that contains a list of [`TableOfContents`]
//! 2) Each [`TableOfContents`] describes a chunk.
//! 3) Each chunk describes either an image or a comment*.
//! 4) Image chunks contain packed ARGB pixels.
//!
//! * Modeling for comment chunks is left out on purpose.

#![allow(unused)] // REMOVE ME LATER

use binrw::BinWrite;

use crate::cursors::common::CursorImage;

/// Converts the bytes in `rgba` to ARGB format in-place.
///
/// ## Panics
///
/// Panics if the length of `rgba` is not a multiple of four.
pub fn to_argb(rgba: &mut [u8]) {
    assert!(
        rgba.len().is_multiple_of(4),
        "invalid RGBA, each pixel should have 4 channels"
    );

    for pixel in rgba.chunks_exact_mut(4) {
        pixel.swap(0, 3); // AGBR
        pixel.swap(1, 2); // ABGR
        pixel.swap(1, 3); // ARGB
    }
}

#[derive(BinWrite)]
#[bw(little, magic = b"Xcur")]
struct XcursorHeader {
    header_size: u32,
    version: u32,
    num_toc: u32,
    tocs: TableOfContents,
}

/// The fields here are specific to image chunks,
/// since I don't plan on adding comment chunk support.
///
/// Only the [`Self::position`] field is useful here,
/// everything else is redundant/duplicated in the chunk.
#[derive(BinWrite)]
#[bw(little, assert(*_type == 0xfffd_0002))]
struct TableOfContents {
    _type: u32,
    /// Also known as the subtype.
    _nominal_size: u32,
    position: u32,
}

/// Prefer using the fields here over the
/// ones stored in [`TableOfContents`].
#[derive(BinWrite)]
#[bw(
    little,
    assert(*header_size == 36),
    assert(*_type == 0xfffd_0002)
)]
struct ImageChunkHeader {
    header_size: u32,
    _type: u32,
    /// Also known as the subtype.
    nominal_size: u32,
    version: u32,
    width: u32,
    height: u32,
    hotspot_x: u32,
    hotspot_y: u32,
    delay: u32,
}

#[derive(BinWrite)]
#[bw(little, assert(argb.len() == (header.width * header.height * 4) as usize))]
struct ImageChunk {
    header: ImageChunkHeader,
    argb: Vec<u8>,
}

impl ImageChunk {
    fn from_cursor_image() {
        todo!();
    }
}
