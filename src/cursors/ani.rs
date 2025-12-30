//! Module for parsing [ANI](https://en.wikipedia.org/wiki/ANI_(file_format)).
//!
//! Note: "CUR" and "ICO" are used interchangeably, the
//! only notable difference is the presence of a hotspot.
//!
//! You may find it helpful to also read about [RIFF](https://en.wikipedia.org/wiki/Resource_Interchange_File_Format).

#![allow(dead_code)]

use binrw::{BinRead, binread};

/* TODO: maybe do some type magic to not duplicate for u8, u32 chunks */

/// RIFF chunk with [`Self::data`] as `Vec<u32>`.
#[binread]
#[derive(Debug)]
#[br(little)]
#[br(import{ expected_id: [u8; 4] })]
pub(super) struct RiffChunkU32 {
    /// ASCII identifier for the chunk.
    #[br(assert(id == expected_id))]
    id: [u8; 4],

    // these fields are temporary since `data`
    // already stores its length when constructed
    //
    // we assert `data_size` is even because
    // `data` is even (4 bytes each)
    //
    // this also means we don't add padding
    #[br(temp)]
    _data_size: u32,
    #[br(try_calc = usize::try_from(_data_size / 4), temp)]
    _data_length: usize,

    /// The chunk data.
    #[br(count = _data_length)]
    pub data: Vec<u32>,
}

/// RIFF chunk with [`Self::data`] as `Vec<u8>`.
///
/// Used for [`RiffListU8`].
#[binread]
#[derive(Debug)]
#[br(little)]
#[br(import{ expected_id: [u8; 4] })]
pub(super) struct RiffChunkU8 {
    /// ASCII identifier for the chunk.
    #[br(assert(id == expected_id))]
    id: [u8; 4],

    // size == length here since `data` is Vec<u8>
    #[br(temp)]
    _data_size: u32,

    /// The chunk data.
    #[br(count = _data_size, pad_after = _data_size % 2)]
    pub data: Vec<u8>,
    // padding byte skipped with `pad_after`
}

#[binread]
#[derive(Debug)]
#[br(import{ list_length: u32, expected_list_id: [u8; 4], expected_subchunk_id: [u8; 4] })]
#[br(assert(list_length != 0))]
#[br(little, magic = b"LIST")]
pub(super) struct RiffListU8 {
    #[br(temp)]
    _list_size: u32,

    #[br(assert(list_id == expected_list_id))]
    list_id: [u8; 4],

    #[br(args {
        count: list_length.try_into().unwrap(),
        inner: RiffChunkU8BinReadArgs { expected_id: expected_subchunk_id }}
    )]
    pub list: Vec<RiffChunkU8>,
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
    UnsequencedIcon = 1,
    /// Contains ICO frames that play in the
    /// order they're defined (no "seq " chunk).
    SequencedIcon = 3,
}

/// Models an ANI file's header (or the "anih" chunk).
///
/// ## References
///
/// - [Wikipedia: ANI structure](https://en.wikipedia.org/wiki/ANI_(file_format)#File_structure)
#[binread]
#[derive(Debug)]
#[br(magic = b"anih")]
pub(super) struct AniHeader {
    /// Size field of the "anih" chunk, not part of the header itself.
    #[br(temp)]
    _anih_size: u32,
    /// The size of this header. Must be 36.
    #[br(assert(_anih_size == _header_size))]
    #[br(assert(_header_size == 36), temp)]
    _header_size: u32,
    /// Number of frames in "fram" LIST.
    ///
    /// This is different from [`Self::num_steps`]:
    ///
    /// ```text
    /// sequence = [0, 1, 2, 1] => num_steps  = 4
    /// frames   = [0, 1, 2]    => num_frames = 3
    /// ```
    pub num_frames: u32,
    /// Number of steps in the animation loop.
    ///
    /// This is different from [`Self::num_frames`]:
    ///
    /// ```text
    /// sequence = [0, 1, 2, 1] => num_steps  = 4
    /// frames   = [0, 1, 2]    => num_frames = 3
    /// ```
    // skip 4 DWORDs (u32s): cx, cy, cBitCount, cPlanes
    // NOTE: don't assert for zero, Windows doesn't care
    #[br(pad_after = 16)]
    pub num_steps: u32,

    /// Default jiffle rate if "rate" isn't provided.
    pub jiffle_rate: u32,
    // Flags to indicate whether the "seq " chunk exists.
    flags: AniFlags,
}

/// This has no fields, doesn't parse metadata,
/// and is only used to skip until [`AniHeader`].
///
/// _no one even writes this chunk anyway..._
#[binread]
#[derive(Debug)]
#[br(magic = b"LIST")]
struct SkipAniMetadata {
    // this chunk (that we're skipping) is just two strings max
    // also, subchunks are even-padded, so the chunk size must be even too
    #[br(assert(_list_size < 1024, "INFO chunk unreasonably large (1KB+)"))]
    #[br(assert(_list_size.is_multiple_of(2)), temp)]
    _list_size: u32,

    // list identifier
    #[br(assert(_info == *b"INFO"), temp)]
    _info: [u8; 4],

    // i wonder if this jump is still done after the size assert
    // even if it fails? oh well, can't be that bad ¯\_(ツ)_/¯

    // -4 since we've read `_info`, which is 4 bytes
    #[br(calc = _list_size.checked_sub(4).unwrap(), temp)]
    _skip_value: u32,

    // just skip all the metadata to jump to `AniHeader`
    #[br(pad_after = _skip_value, temp)]
    _skip: (),

    // make sure we skipped far enough
    #[br(assert(_anih == *b"anih"), temp)]
    _anih: [u8; 4],

    // step back so `AniHeader` can assert its magic
    #[br(pad_after = -4, temp)]
    _back: (),
}

/// Models an ANI file.
#[binread]
#[derive(Debug)]
#[br(little, magic = b"RIFF")]
#[br(assert(
    (header.flags == AniFlags::UnsequencedIcon && sequence.is_none())
     ||(header.flags == AniFlags::SequencedIcon && sequence.is_some())
    ),
)]
pub(super) struct AniFile {
    #[br(assert(file_size < 1_048_576, "file_size unreasonably large (1MB+)"))]
    pub file_size: u32,

    #[br(assert(_acon == *b"ACON"), temp)]
    _acon: [u8; 4],
    #[br(try, temp)]
    _metadata: Option<SkipAniMetadata>,

    pub header: AniHeader,

    #[br(try, args{ expected_id: *b"rate" })]
    pub rate: Option<RiffChunkU32>,

    #[br(try, args{ expected_id: *b"seq " })]
    pub sequence: Option<RiffChunkU32>,

    #[br(args {
        list_length: header.num_frames,
        expected_list_id: *b"fram", 
        expected_subchunk_id: *b"icon" }
    )]
    pub frames: RiffListU8,
}
