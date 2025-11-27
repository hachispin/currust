//! Contains structs that model byte layouts of
//! the [Xcursor](https://man.archlinux.org/man/Xcursor.3) format.
//!
//! The Xcursor format generally follows this structure:
//!
//! 1) File header that contains a list of [`TableOfContents`]
//! 2) Each [`TableOfContents`] describes a chunk.
//! 3) Each chunk describes either an image or a comment.
//! 4) Image chunks contain packed ARGB pixels.

#![allow(unused)] // REMOVE ME LATER

use binrw::BinWrite;

/// Converts the bytes in `rgba` to ARGB format in-place.
/// 
/// ## Panics
/// 
/// Panics if the length of `rgba` is not a multiple of four.
pub fn to_argb(rgba: &mut [u8]) {
    assert!(
        !rgba.len().is_multiple_of(4),
        "invalid RGBA, each pixel should have 4 channels"
    );

    for pixel in rgba.chunks_exact_mut(4) {
        pixel.swap(0, 3); // AGBR
        pixel.swap(1, 2); // ABGR
        pixel.swap(1, 3); // ARGB
    }
}

/// The header for the Xcursor file format.
#[derive(BinWrite)]
#[bw(little, magic = b"Xcur")]
struct XcursorHeader {
    /// The size of this header.
    header_size: u32,
    /// The file version number.
    version: u32,
    /// The number of [`TableOfContents`] entries.
    num_toc: u32,
    /// A list of [`TableOfContents`].
    toc_entries: Vec<TableOfContents>,
}

#[derive(BinWrite)]
#[bw(little)]
struct TableOfContents {
    type_: u32,
    subtype: u32,
    position: u32,
}

#[derive(BinWrite)]
#[bw(little)]
struct ChunkHeader {
    header_size: u32,
    /// Must match corresponding TOC type.
    type_: u32,
    /// Must match corresponding TOC subtype.
    subtype: u32,
    version: u32,
}

#[derive(BinWrite)]
#[bw(little)]
struct CommentChunk {
    header: ChunkHeader,
    length: u32,
    /* string: String */
}

#[derive(BinWrite)]
#[bw(little)]
struct ImageChunk {
    header: ChunkHeader,
    width: u32,
    height: u32,
    hotspot_x: u32,
    hotspot_y: u32,
    delay: u32,
    argb: Vec<u8>,
}
