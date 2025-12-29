//! Module for parsing [ANI](https://en.wikipedia.org/wiki/ANI_(file_format)).
//!
//! Note: "CUR" and "ICO" are used interchangeably, the
//! only notable difference is the presence of a hotspot.
//!
//! You may find it helpful to also read about [RIFF](https://en.wikipedia.org/wiki/Resource_Interchange_File_Format).

#![allow(unused)] // suppress for now

use super::generic_cursor::GenericCursor;

use std::{fs, path::Path};

use anyhow::{Context, Result};
use binrw::BinRead;
use ico::IconDir;

/// All chunks follow this structure:
///
/// - 4 bytes: an ASCII identifier (e.g, "fmt ", "data"; note the space in "fmt ").
/// - 4 bytes: a little-endian u32, `n`, which is the size of the next field.
/// - `n` bytes: the chunk data itself, of the size, `n`, given previously.
/// - a padding byte, if `n` is odd.
///
/// ## References
///
/// - [Wikipedia: RIFF explanation](https://en.wikipedia.org/wiki/Resource_Interchange_File_Format#Explanation)
#[derive(Debug, BinRead)]
struct RiffChunk {
    /// ASCII identifier for the chunk.
    id: [u8; 4],
    /// Size of [`Self::data`] in bytes.
    data_size: u32,
    /// The chunk data.
    #[br(count = data_size, pad_after = data_size % 2)]
    data: Vec<u8>,
    // padding byte is skipped with `pad_after`
}

#[derive(Debug, BinRead)]
#[br(
    magic = b"LISTfram", // hack
    assert(frames.iter().all(|f| f.id == *b"icon")),
)]
struct AniFrames {
    num_frames: u32,
    #[br(count=num_frames)]
    frames: Vec<RiffChunk>,
}

/// Contains possible flag combinations for [`AniHeader`].
/// These are used to describe the state of the "seq " chunk.
///
/// Invalid flags:
///
/// - `0x0`: no flags set
/// - `0x2`: frames are not ICO
///
/// All frames must be in ICO format.
#[derive(Debug, PartialEq, BinRead)]
#[br(repr = u32)]
enum AniFlags {
    /// Contains ICO frames with a custom "seq " chunk,
    /// which defines the order frames should be played.
    ///
    /// This is mainly for optimizing repeated frames(?).
    SequencedIcon = 0x1,
    /// Contains ICO frames that play in the
    /// order they're defined (no "seq " chunk).
    UnsequencedIcon = 0x3,
}

/// Models an ANI file's header.
///
/// ## Refernces
///
/// - [Wikipedia: ANI structure](https://en.wikipedia.org/wiki/ANI_(file_format)#File_structure)
#[derive(Debug, BinRead)]
#[br(magic = b"anih")]
struct AniHeader {
    header_size: u32,
    num_frames: u32,
    // skip four DWORDs for unused fields: cx, cy, cBitCount, cPlanes
    #[br(pad_after = size_of::<u32>() * 4)]
    num_steps: u32,
    jiffle_rate: u32,
    flags: AniFlags,
}

#[derive(Debug, BinRead)]
#[br(magic = b"RIFF")]
struct AniFile {/* TODO: Finish this */}
