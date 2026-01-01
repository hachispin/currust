//! Module for parsing [ANI](https://en.wikipedia.org/wiki/ANI_(file_format)).
//!
//! Note: "CUR" and "ICO" are used interchangeably, the
//! only notable difference is the presence of a hotspot.
//!
//! You may find it helpful to also read about [RIFF](https://en.wikipedia.org/wiki/Resource_Interchange_File_Format).

use std::io::Cursor;

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

/// Models an ANI file's header (or the "anih" chunk).
///
/// ## References
///
/// - [Wikipedia: ANI structure](https://en.wikipedia.org/wiki/ANI_(file_format)#File_structure)
#[binread]
#[derive(Debug, PartialEq)]
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

impl Default for AniHeader {
    fn default() -> Self {
        Self {
            num_frames: 0,
            num_steps: 0,
            jiffy_rate: 0,
            flags: AniFlags::SequencedIcon,
        }
    }
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
        let mut ani = AniFile::default();

        // make window for matching identifiers/fourcc (e.g, "seq ")
        // NOTE: consider using array_windows when it's stable
        let windows = ani_blob.windows(4);

        for (offset, id) in windows.enumerate() {
            // there are asserts here checking for duplicate chunks
            // i'll turn them into results if this ever panics though

            match id {
                b"LIST" => {
                    let list_id = &ani_blob[(offset + 8)..(offset + 12)];

                    match list_id {
                        b"INFO" => {
                            eprintln!("Found 'INFO' chunk at offset={offset}, skipping");
                        }

                        b"fram" => {
                            assert!(ani.ico_frames.is_empty());
                            ani.ico_frames = Self::parse_fram(ani_blob, offset)?;
                        }

                        _ => bail!("Unexpected 'LIST' subtype {list_id:?}"),
                    }
                }

                b"anih" => {
                    assert!(ani.header == AniHeader::default());
                    ani.header = Self::parse_anih(ani_blob, offset)?;
                }
                
                b"rate" => {
                    assert!(ani.rate.is_none());
                    ani.rate = Some(Self::parse_rate(ani_blob, offset)?);
                }

                b"seq " => {
                    assert!(ani.sequence.is_none());
                    ani.sequence = Some(Self::parse_seq(ani_blob, offset)?);
                }
                _ => (),
            }
        }

        /* check invariants */

        let hdr = &ani.header;

        if (hdr.flags == AniFlags::SequencedIcon && ani.sequence.is_none())
            || (hdr.flags == AniFlags::UnsequencedIcon && ani.sequence.is_some())
        {
            bail!(
                "flags didn't match state of 'seq ' chunk: flags={:?}, sequence={:?}",
                hdr.flags,
                ani.sequence
            );
        }

        if hdr.num_frames != ani.ico_frames.len().try_into()? {
            bail!(
                "expected {} frames, instead got {}",
                hdr.num_frames,
                ani.ico_frames.len()
            );
        }

        Ok(ani)
    }

    /// Helper for [`Self::from_blob`] for the "fram" chunk.
    ///
    /// Note: `offset` is where the "LIST" fourcc starts.
    fn parse_fram(ani_blob: &[u8], offset: usize) -> Result<Vec<RiffChunkU8>> {
        let fram_chunk = Self::extract_chunk(ani_blob, offset)?;
        let mut chunks = Vec::new();

        // not this again...
        for (fram_offset, fourcc) in fram_chunk.windows(4).enumerate() {
            if fourcc != b"icon" {
                continue;
            }

            let icon_chunk = Self::extract_chunk(ani_blob, offset + fram_offset)?;
            let args = RiffChunkU8BinReadArgs {
                expected_id: *b"icon",
            };
            let parsed_chunk = RiffChunkU8::read_le_args(&mut Cursor::new(icon_chunk), args)
                .context("failed to parse 'icon' subchunk in 'fram'")?;

            chunks.push(parsed_chunk);
        }

        Ok(chunks)
    }

    fn parse_anih(ani_blob: &[u8], offset: usize) -> Result<AniHeader> {
        let anih_chunk = Self::extract_chunk(ani_blob, offset)?;
        AniHeader::read_le(&mut Cursor::new(anih_chunk)).context("failed to parse 'anih' chunk")
    }

    fn parse_rate(ani_blob: &[u8], offset: usize) -> Result<RiffChunkU32> {
        let rate_chunk = Self::extract_chunk(ani_blob, offset)?;
        let args = RiffChunkU32BinReadArgs {
            expected_id: *b"rate",
        };

        RiffChunkU32::read_le_args(&mut Cursor::new(rate_chunk), args)
            .context("failed to parse 'rate' chunk")
    }

    fn parse_seq(ani_blob: &[u8], offset: usize) -> Result<RiffChunkU32> {
        let seq_chunk = Self::extract_chunk(ani_blob, offset)?;
        let args = RiffChunkU32BinReadArgs {
            expected_id: *b"seq ",
        };

        RiffChunkU32::read_le_args(&mut Cursor::new(seq_chunk), args)
            .context("failed to parse 'seq ' chunk")
    }

    /// Calculates the slice that contains the chunk.
    /// `chunk_start` is where the fourcc starts.
    fn extract_chunk(blob: &[u8], chunk_start: usize) -> Result<&[u8]> {
        let blob_len = blob.len();

        if chunk_start + 8 > blob.len() {
            bail!("blob.len()={blob_len} too small to hold chunk_start={chunk_start}");
        }

        // size always comes after identifier for RIFF chunks
        let chunk_size_u32 =
            u32::from_le_bytes(blob[(chunk_start + 4)..(chunk_start + 8)].try_into()?);
        let chunk_size = usize::try_from(chunk_size_u32)?;
        let chunk_end = chunk_start + chunk_size + 8;

        if chunk_end > blob.len() {
            bail!("chunk_end={chunk_end} greater than blob.len()={blob_len}");
        }

        let chunk = &blob[chunk_start..(chunk_start + chunk_size + 8)];

        Ok(chunk)
    }
}
