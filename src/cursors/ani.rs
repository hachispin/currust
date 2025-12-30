//! Module for parsing [ANI](https://en.wikipedia.org/wiki/ANI_(file_format)).
//!
//! Note: "CUR" and "ICO" are used interchangeably, the
//! only notable difference is the presence of a hotspot.
//!
//! You may find it helpful to also read about [RIFF](https://en.wikipedia.org/wiki/Resource_Interchange_File_Format).

#![allow(unused)] // suppress for now

use super::generic_cursor::GenericCursor;

use std::{fs, io::SeekFrom, path::Path};

use anyhow::{Context, Result};
use binrw::{BinRead, binread};
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

/// Contains possible flag combinations for [`AniHeader`].
/// These are used to describe the state of the "seq " chunk.
///
/// Invalid flags:
///
/// - `0`: no flags set
/// - `2`: frames are not ICO
///
/// All frames must be in ICO format.
#[derive(Debug, PartialEq, BinRead)]
#[br(repr = u32)]
enum AniFlags {
    /// Contains ICO frames with a custom "seq " chunk,
    /// which defines the order frames should be played.
    ///
    /// This is mainly for optimizing repeated frames(?).
    SequencedIcon = 1,
    /// Contains ICO frames that play in the
    /// order they're defined (no "seq " chunk).
    UnsequencedIcon = 3,
}

/// Models an ANI file's header.
///
/// ## References
///
/// - [Wikipedia: ANI structure](https://en.wikipedia.org/wiki/ANI_(file_format)#File_structure)
#[binread]
#[derive(Debug)]
#[br(magic = b"anih")]
#[br( // reference:  https://www.gdgsoft.com/anituner/help/aniformat.htm
    assert(_cx == 0 && _cy == 0 && _c_bit_count == 0 && _c_planes == 0,
        "_cx, _cy, _c_bit_count and _c_planes are reserved and must be 0"
    ),
)]
struct AniHeader {
    header_size: u32,
    num_frames: u32,
    num_steps: u32,

    #[br(temp)]
    _cx: u32,
    #[br(temp)]
    _cy: u32,
    #[br(temp)]
    _c_bit_count: u32,
    #[br(temp)]
    _c_planes: u32,

    jiffle_rate: u32,
    flags: AniFlags,
}

/// This has no fields, doesn't parse metadata,
/// and is only used to skip until [`AniHeader`].
///
/// _no one even writes this chunk anyway..._
#[binread]
#[derive(Debug)]
#[br(magic = b"INFO")]
struct SkipAniMetadata {
    // this chunk (that we're skipping) is just two strings max, so
    #[br(assert(_list_size < 1024, "INFO chunk unreasonably large"), temp)]
    _list_size: u32,

    // just skip all the metadata to jump to `AniHeader`
    // RIFF adds padding to make chunk sizes even
    #[br(calc = _list_size % 2, temp)]
    _padding: u32,
    #[br(pad_after = (_list_size + _padding), temp)]
    _skip: (),

    // make sure we skipped far enough, to `AniHeader`
    #[br(assert(_anih == *b"anih"), temp)]
    _anih: [u8; 4],

    // step back so `AniHeader` can assert its magic
    #[br(pad_after = -4, temp)]
    _back: (),
}

#[binread]
#[derive(Debug)]
#[br(magic = b"RIFFACON")]
struct AniFile {
    first_id: [u8; 4],
    #[br(if(first_id == *b"LIST"), temp)]
    _metadata: Option<SkipAniMetadata>,
    header: AniHeader,
    second_id: [u8; 4],
    /* TODO: Finish this */
}
