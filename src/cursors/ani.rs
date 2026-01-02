//! Module for parsing [ANI](https://en.wikipedia.org/wiki/ANI_(file_format)).
//!
//! Note: "CUR" and "ICO" are used interchangeably, the
//! only notable difference is the presence of a hotspot.
//!
//! You may find it helpful to also read about [RIFF](https://en.wikipedia.org/wiki/Resource_Interchange_File_Format).

use std::io::{Cursor, Read, Seek};

use anyhow::{Context, Result, bail};
use binrw::{BinRead, binread};

/// RIFF chunk with [`Self::data`] as `Vec<u32>`.
#[binread]
#[derive(Debug)]
#[br(little)]
#[br(import{ expected_id: [u8; 4] })]
pub(super) struct RiffChunkU32 {
    /// ASCII identifier for the chunk.
    #[br(assert(_id == expected_id))]
    _id: [u8; 4],

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

/// RIFF chunk with [`Self::data`] as `Vec<u8>`.
///
/// Used for [`RiffListU8`].
#[binread]
#[derive(Debug)]
#[br(little)]
#[br(import{ expected_id: [u8; 4] })]
pub(super) struct RiffChunkU8 {
    /// ASCII identifier for the chunk.
    #[br(assert(_id == expected_id))]
    _id: [u8; 4],

    // size == length here since `data` is Vec<u8>
    #[br(temp)]
    _data_size: u32,

    /// The chunk data.
    #[br(count = _data_size, pad_after = _data_size % 2)]
    pub data: Vec<u8>,
    // padding byte skipped with `pad_after`
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
    #[br(
        try_calc =
            _list_size.checked_sub(4)
            .ok_or_else(|| format!("overflow on list_size={_list_size} - 4")),
        temp
    )]
    _skip_value: u32,

    // yolo
    #[br(pad_after = _skip_value)]
    _skip: (),
}

/// Contains possible flag combinations for [`AniHeader`].
/// These are used to describe the state of the "seq " chunk.
///
/// The ANI format defines these flags:
///
/// ```text
/// #define AF_ICON 0x1         // Frames are in Windows ICO format.
/// #define AF_SEQUENCE 0x2     // Animation is sequenced.
/// ```
///
/// All frames must be in ICO format, so these are invalid flags:
///
/// - `0`: no flags set
/// - `2`: frames are not ICO
#[derive(Debug, Default, PartialEq, BinRead)]
#[br(repr = u32)]
enum AniFlags {
    // NOTE: this is storing the valid combinations of
    // bitflags and are not meant to be composable.
    /// Contains ICO frames with a custom "seq " chunk,
    /// which defines the order frames should be played.
    ///
    /// This is mainly for optimizing repeated frames(?).
    #[default]
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
#[derive(Debug, Default, PartialEq)]
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
    #[br(pad_after = 16)] // contains unused fields: cx, cy, cBitCount, cPlanes
    pub num_steps: u32,

    /// Default jiffy rate if "rate" isn't provided.
    pub jiffy_rate: u32,
    // Flags to indicate whether the "seq " chunk exists.
    flags: AniFlags,
}

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
/// NOTE: The order shown here may not reflect how actual ANI files
///       can choose to order their fields.
///
/// - Chunks always follow this: identifier => data size => even-padded data.
///   * Data size doesn't include padding.
/// - Brackets around a chunk (like "seq ") indicate that it's optional.
/// - Chunks like "RIFF" and "LIST" have a second identifier, after the size.
///
/// ## References
///
/// [Wikipedia: ANI structure](https://en.wikipedia.org/wiki/ANI_(file_format)#File_structure)
#[derive(Debug, Default)]
pub(super) struct AniFile {
    pub header: AniHeader,
    pub rate: Option<RiffChunkU32>,
    pub sequence: Option<RiffChunkU32>,
    pub ico_frames: Vec<RiffChunkU8>,
}

impl AniFile {
    /// Parses `ani_blob`.
    ///
    /// This is pretty complicated (uses a sliding window)
    /// because of the "constraint" (or more like freedom?)
    /// of chunks being able to appear in any order.
    ///
    /// > [gdgsoft](https://www.gdgsoft.com/anituner/help/aniformat.htm):
    /// > "Any of the blocks ("ACON", "anih", "rate", or "seq ") can appear in any order."
    // this is the worst format i've ever seen
    pub fn from_blob(ani_blob: &[u8]) -> Result<AniFile> {
        const MAX_RIFF_SIZE: usize = 2_097_152;

        if ani_blob.len() > MAX_RIFF_SIZE {
            bail!(
                "ani_blob.len()={} unreasonably large (2MB+)",
                ani_blob.len()
            )
        }

        // for sanity checks against read sizes
        let ani_blob_len_u64 = u64::try_from(ani_blob.len())?;
        let mut ani = AniFile::default();
        let mut cursor = Cursor::new(ani_blob);
        let mut buf = [0u8; 4];
        cursor.read_exact(&mut buf)?;

        if buf != *b"RIFF" {
            bail!("expected 'RIFF' chunk, instead got {buf:?}");
        }

        cursor.read_exact(&mut buf)?;
        let riff_size = u32::from_le_bytes(buf);

        // NOTE: stricter checks like this fail on "valid" files
        // `riff_size == blob.len() - 8`
        // https://github.com/quantum5/win2xcur/commit/ac9552ce83d2955a96a4d7a5cfde7c113ec5a4c5
        if u64::from(riff_size) > ani_blob_len_u64 {
            bail!("riff_size={riff_size} extends beyond blob")
        }

        cursor.read_exact(&mut buf)?;

        if buf != *b"ACON" {
            bail!("expected 'ACON' as 'RIFF' subtype, instead got {buf:?}");
        }

        // read chunks and parse
        while cursor.position() < ani_blob.len().try_into()? {
            cursor.read_exact(&mut buf)?;

            // deref patterns are unstable
            match &buf {
                b"LIST" => Self::parse_list(&mut cursor, &mut ani)?,
                b"anih" => {
                    if ani.header != AniHeader::default() {
                        bail!("duplicate 'anih' chunk");
                    }

                    ani.header = Self::parse_anih(&mut cursor)?;
                }

                b"rate" => {
                    ani.rate = {
                        if ani.rate.is_some() {
                            bail!("duplicate 'rate' chunk");
                        }

                        Some(Self::parse_rate(&mut cursor)?)
                    }
                }

                b"seq " => {
                    if ani.sequence.is_some() {
                        bail!("duplicate 'seq ' chunk");
                    }

                    ani.sequence = Some(Self::parse_seq(&mut cursor)?);
                }

                // consider attempting to read size and skipping
                // for unknown chunks (but it's a bit unreliable)
                _ => bail!("Unexpected fourcc(?) buf={buf:?}"),
            }
        }

        /* check "invariants" */
        // if something isn't a bail!(), it's for good reason
        // windows still renders some technically invalid files
        let hdr = &ani.header;

        if hdr.flags == AniFlags::SequencedIcon && ani.sequence.is_none() {
            eprintln!(
                "Warning: expected 'seq ' chunk from flags={:?}, found None",
                hdr.flags
            );
        }

        if hdr.flags == AniFlags::UnsequencedIcon && ani.sequence.is_some() {
            eprintln!(
                "Warning: expected 'seq ' chunk to be None from flags={:?}, found sequence={:?}",
                hdr.flags, ani.sequence
            );
        }

        if usize::try_from(hdr.num_frames)? != ani.ico_frames.len() {
            bail!(
                "Warning: expected num_frames={}, instead got ico_frames.len()={}",
                hdr.num_frames,
                ani.ico_frames.len()
            );
        }

        if let Some(seq) = &ani.sequence
            && seq.data.iter().max() >= Some(&hdr.num_frames)
        {
            bail!("frame indices of 'seq ' chunk go out of bounds");
        }

        Ok(ani)
    }

    /// Helper for [`Self::from_blob`] for the "LIST" chunk.
    ///
    /// This can diverge depending on the subtype, which can
    /// either be "INFO" (skipped) or "fram" (frame data).
    fn parse_list(cursor: &mut Cursor<&[u8]>, ani: &mut AniFile) -> Result<()> {
        const MAX_FRAM_SIZE: u32 = 1_048_576; // a megabyte

        let ani_blob_size = cursor.get_ref().len();
        let mut buf = [0u8; 4];
        cursor.read_exact(&mut buf)?;
        let list_size = u32::from_le_bytes(buf);
        let mut list_id = [0u8; 4];
        cursor.read_exact(&mut list_id)?;

        match &list_id {
            b"INFO" => {
                SkipAniMetadata::read_le(cursor).context("failed to skip 'INFO' chunk")?;
            }

            b"fram" => {
                if !ani.ico_frames.is_empty() {
                    bail!("duplicate 'fram' chunk");
                }

                let mut chunks: Vec<RiffChunkU8> = Vec::new();
                let fram_size = list_size
                    .checked_sub(4)
                    .with_context(|| format!("underflow on list_size={list_size} - 4"))?;

                if fram_size > MAX_FRAM_SIZE {
                    bail!("fram_size={fram_size} unreasonably large (1MB+)");
                }

                let end = cursor
                    .position()
                    .checked_add(u64::from(fram_size))
                    .with_context(|| {
                        format!(
                            "overflow on cursor.position={} + fram_size={fram_size}",
                            cursor.position()
                        )
                    })?;

                // if we read `fram_size` bytes, are we still in the blob?
                if end > ani_blob_size.try_into()? {
                    bail!("fram_size={fram_size} extends beyond blob");
                }

                while cursor.position() < end {
                    const ICON_ARGS: RiffChunkU8BinReadArgs = RiffChunkU8BinReadArgs {
                        expected_id: *b"icon",
                    };

                    let chunk = RiffChunkU8::read_le_args(cursor, ICON_ARGS)
                        .context("failed to read 'icon' subchunk of 'fram'")?;

                    chunks.push(chunk);
                }

                if chunks.is_empty() {
                    bail!("Failed to parse any frames from 'fram' chunk");
                }

                ani.ico_frames = chunks;
            }

            _ => bail!("Unexpected list_id={list_id:?}"),
        }

        Ok(())
    }

    /// Helper for [`Self::from_blob`] for the "anih" chunk.
    #[inline]
    fn parse_anih(cursor: &mut Cursor<&[u8]>) -> Result<AniHeader> {
        // step back so `AniHeader` can assert magic ("anih")
        cursor.seek_relative(-4)?;

        AniHeader::read_le(cursor).context("failed to read 'anih' chunk")
    }

    /// Helper for [`Self::from_blob`] for the "rate" chunk.
    #[inline]
    fn parse_rate(cursor: &mut Cursor<&[u8]>) -> Result<RiffChunkU32> {
        const RATE_ARGS: RiffChunkU32BinReadArgs = RiffChunkU32BinReadArgs {
            expected_id: *b"rate",
        };

        cursor.seek_relative(-4)?;
        RiffChunkU32::read_le_args(cursor, RATE_ARGS).context("failed to read 'rate' chunk")
    }

    /// Helper for [`Self::from_blob`] for the "seq " chunk.
    #[inline]
    fn parse_seq(cursor: &mut Cursor<&[u8]>) -> Result<RiffChunkU32> {
        const SEQ_ARGS: RiffChunkU32BinReadArgs = RiffChunkU32BinReadArgs {
            expected_id: *b"seq ",
        };

        cursor.seek_relative(-4)?;
        RiffChunkU32::read_le_args(cursor, SEQ_ARGS).context("failed to read 'seq ' chunk")
    }
}
