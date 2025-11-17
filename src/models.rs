//! Contains structs modeling byte layouts of Windows cursors.
//!
//! Note that the `.cur` format follows little-endian byte ordering.

use std::{fs, path::Path};

use miette::{IntoDiagnostic, Result};
use zerocopy::{
    FromBytes, Immutable, KnownLayout,
    little_endian::{U16 as u16_le, U32 as u32_le},
};

#[repr(C, packed)]
#[derive(FromBytes, KnownLayout, Immutable, Debug)]
/// Partially models the byte layout of `ICONDIR`.
///
/// This does not contain `idEntries` due to it
/// being variable-sized (with length `img_count`).
///
/// Reference: https://en.wikipedia.org/wiki/ICO_(file_format)#ICONDIR_structure
struct IconDirHeader<'a> {
    /// must be 0
    _reserved: &'a u16_le,
    /// 1 = ICO, 2 = CUR. other values are invalid
    type_: &'a u16_le,
    img_count: &'a u16_le,
}

/// The full model of `ICONDIR`, including `idEntries`
///
/// Lifetime `'b` indicates that the blob these fields
/// point to must be valid for as long as [`IconDir`] is.
pub struct IconDir<'a> {
    _reserved: &'a u16_le,
    type_: &'a u16_le,
    img_count: &'a u16_le,
    img_entries: &'a [IconDirEntry<'a>],
}

// impl IconDir {
//     /// Parses `bytes` and does any dirty work.
//     ///
//     /// Use this over [`IconDirHeader::read_from_bytes`].
//     pub fn new(bytes: &[u8]) -> Result<Self> {
//         let (header, entries) = IconDirHeader::read_from_prefix(bytes).into_diagnostic()?;

//         let entry_size = size_of::<IconDirEntry>();
//         assert_eq!(entry_size, 16);
//         let end = usize::from(header.img_count) * entry_size;
//         let entries: Vec<IconDirEntry> = Vec::with_capacity(usize::from(header.img_count));

//         panic!();
//     }
// }

#[repr(C, packed)]
#[derive(FromBytes, KnownLayout, Immutable, Debug)]
/// Models the byte layout of `ICONDIRENTRY`, which
/// stores info regarding an image (one image only)
///
/// Reference: https://en.wikipedia.org/wiki/ICO_(file_format)#ICONDIRENTRY_structure
pub struct IconDirEntry<'b> {
    /// width of stored image
    width: u8,
    /// height of stored image
    height: u8,
    /// irrelevant really
    color_count: u8,
    /// must be 0
    _reserved: u8,
    // x-coordinate of cursor click pixel (i.e, hotspot)
    hotspot_x: &'b u16_le,
    /// y-coordinate of cursor click pixel (i.e, hotspot)
    hotspot_y: &'b u16_le,
    /// size of stored image in bytes
    img_size: &'b u32_le,
    /// offset from start of `.cur` blob where png/bmp data starts
    img_offset: &'b u32_le,
}

struct WinCursor<'blob> {
    blob: Vec<u8>,
    /// lives as long as WinCursor so blob stays usable
    icon_dir: IconDir<'blob>,
}

impl WinCursor<'_> {
    pub fn new(fp: &Path) -> Result<Self> {
        let bytes = fs::read(fp).into_diagnostic()?;

        todo!();
    }
}
