//! Contains structs modeling byte layouts of Windows cursors.
//!
//! Note that `.cur` follow little-endian byte ordering.

use zerocopy::{
    FromBytes, Immutable, KnownLayout,
    little_endian::{U16 as u16_le, U32 as u32_le},
};

#[repr(C, packed)]
#[derive(FromBytes, KnownLayout, Immutable, Debug)]
/// Models the byte layout of `ICONDIRENTRY`, which
/// stores info regarding an image (one image only)
///
/// Reference: https://en.wikipedia.org/wiki/ICO_(file_format)#ICONDIRENTRY_structure
pub struct IconDirEntry {
    /// width of stored image
    width: u8,
    /// height of stored image
    height: u8,
    /// irrelevant really
    color_count: u8,
    /// do not touch. must be 0--used for assertions
    _reserved: u8,
    // x-coordinate of cursor click pixel (i.e, hotspot)
    hotspot_x: u16_le,
    /// y-coordinate of cursor click pixel (i.e, hotspot)
    hotspot_y: u16_le,
    /// size of stored image in bytes
    img_size: u32_le,
    /// offset from start of `.cur` blob where png/bmp data starts
    img_offset: u32_le,
}
