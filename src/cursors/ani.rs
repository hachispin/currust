//! Module for parsing [ANI](https://en.wikipedia.org/wiki/ANI_(file_format)).
//!
//! Note: "CUR" and "ICO" are used interchangeably, the
//! only notable difference is the presence of a hotspot.
//!
//! You may find it helpful to also read about [RIFF](https://en.wikipedia.org/wiki/Resource_Interchange_File_Format).

use binrw::{BinRead, binread};

/// Models an ANI file.
///
/// ```text
/// RIFF('ACON'
///     [LIST('INFO'                   
///         [INAM(<ZSTR>)]             // Title. Optional.
///         [IART(<ZSTR>)]             // Author. Optional.
///     )]                             
///     'anih'(<ANIHEADER>)            // ANI file header.
///     ['rate'(<DWORD...>)]           // Rate table (array of jiffies).
///                                    // If the AF_SEQUENCE flag is set
///                                    // then the count is ANIHEADER.cSteps,
///                                    // otherwise ANIHEADER.cFrames.
///     ['seq '(<DWORD...>)]           // Sequence table (array of frame index values).
///                                    // Should be present when AF_SEQUENCE flag is set.
///                                    // Count is ANIHEADER.cSteps.
///     LIST('fram'                    // List of frames data. Count is ANIHEADER.cFrames.
///        'icon'(<icon_data_1>)       // Frame 1
///        'icon'(<icon_data_2>)       // Frame 2
///        ...
///     )
/// )
/// ```
///
/// - Chunks always follow this: identifier => data size => even-padded data.
///   * Data size doesn't include padding.
/// - Brackets around a chunk (like "seq ") indicate that it's optional.
/// - Chunks like "RIFF" and "LIST" have a second identifier, after the size.
///
/// ## References
///
/// [Wikipedia: ANI structure](https://en.wikipedia.org/wiki/ANI_(file_format)#File_structure)
#[binread]
#[derive(Debug)]
#[br(little, magic = b"RIFF")]
#[br(assert(
    (header.flags == AniFlags::UnsequencedIcon && sequence.is_none()) ||
    (header.flags == AniFlags::SequencedIcon /* && sequence.is_some() */)
    ), //                                    └────────────┬────────────┘
)] //                        some cursors don't follow this but still render on windows
pub(super) struct AniFile {
    #[allow(dead_code)]
    #[br(assert(file_size < 1_048_576, "file_size unreasonably large (1MB+)"))]
    pub file_size: u32,

    #[br(assert(_acon == *b"ACON"), temp)]
    _acon: [u8; 4],
    #[br(try, temp)]
    _metadata: Option<SkipAniMetadata>,
    pub header: AniHeader,

    #[allow(dead_code)]
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

    /// Default jiffy rate if "rate" isn't provided.
    pub jiffy_rate: u32,
    // Flags to indicate whether the "seq " chunk exists.
    flags: AniFlags,
}

/// Contains possible flag combinations for [`AniHeader`].
/// These are used to describe the state of the "seq " chunk.
///
/// The ANI format defines these flags:
///
///```text
/// #define AF_ICON 0x1         // Frames are in Windows ICO format.
/// #define AF_SEQUENCE 0x2     // Animation is sequenced.
/// ```
///
/// All frames must be in ICO format, so these are invalid flags:
///
/// - `0`: no flags set
/// - `2`: frames are not ICO
#[derive(Debug, PartialEq, BinRead)]
#[br(repr = u32)]
enum AniFlags {
    // NOTE: this is storing the valid combinations of
    // bitflags and are not meant to be composable.
    /// Contains ICO frames with a custom "seq " chunk,
    /// which defines the order frames should be played.
    ///
    /// This is mainly for optimizing repeated frames(?).
    UnsequencedIcon = 1,
    /// Contains ICO frames that play in the
    /// order they're defined (no "seq " chunk).
    SequencedIcon = 3,
}

/// This has no fields, doesn't parse metadata,
/// and is only used to skip until [`AniHeader`].
///
/// _no one even writes this chunk anyway..._
#[binread]
#[derive(Debug)]
#[br(magic = b"LIST")]
struct SkipAniMetadata {
    /* TODO: consider actually parsing this */
    // this chunk (that we're skipping) is just two strings max
    // also, subchunks are even-padded, so the chunk size must be even too
    #[br(
        assert(_list_size < 1024, "INFO chunk unreasonably large (1KB+)"),
        assert(_list_size.is_multiple_of(2)), temp
    )]
    _list_size: u32,

    // list identifier
    #[br(assert(_info == *b"INFO"), temp)]
    _info: [u8; 4],

    // -4 since we've read `_info`, which is 4 bytes
    #[br(calc = _list_size.checked_sub(4).unwrap(), temp)]
    _skip_value: u32,

    // make sure we skipped far enough
    #[br(pad_before = _skip_value, restore_position, temp)]
    #[br(assert(_anih == *b"anih"))]
    _anih: [u8; 4],
}

/// A generic LIST chunk with subchunks that store u8 bytes.
#[binread]
#[derive(Debug)]
#[br(import{ list_length: u32, expected_list_id: [u8; 4], expected_subchunk_id: [u8; 4] })]
#[br(assert(list_length != 0))]
#[br(little, magic = b"LIST")]
pub(super) struct RiffListU8 {
    #[br(temp)]
    _list_size: u32,

    #[allow(dead_code)]
    #[br(assert(list_id == expected_list_id))]
    list_id: [u8; 4],

    #[br(args {
        count: list_length.try_into().unwrap(),
        inner: RiffChunkU8BinReadArgs { expected_id: expected_subchunk_id }}
    )]
    pub list: Vec<RiffChunkU8>,
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
    #[allow(dead_code)]
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

/// RIFF chunk with [`Self::data`] as `Vec<u32>`.
#[binread]
#[derive(Debug)]
#[br(little)]
#[br(import{ expected_id: [u8; 4] })]
pub(super) struct RiffChunkU32 {
    /// ASCII identifier for the chunk.
    #[allow(dead_code)]
    #[br(assert(id == expected_id))]
    id: [u8; 4],

    // these fields are temporary since `data`
    // already stores its length when constructed
    //
    // we assert `data_size` is even because
    // `data` is even (4 bytes each)
    //
    // this also means we don't add padding
    #[br(temp, assert(_data_size.is_multiple_of(2)))]
    _data_size: u32,
    #[br(try_calc = usize::try_from(_data_size / 4), temp)]
    _data_length: usize,

    /// The chunk data.
    #[br(count = _data_length)]
    pub data: Vec<u32>,
}
