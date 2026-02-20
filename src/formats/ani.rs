//! Module for parsing [ANI](https://en.wikipedia.org/wiki/ANI_(file_format)).
//!
//! Note: "CUR" and "ICO" are used interchangeably, the
//! only notable difference is the presence of a hotspot.
//!
//! You may find it helpful to also read about [RIFF](https://en.wikipedia.org/wiki/Resource_Interchange_File_Format).

use std::{
    fmt,
    io::{Cursor, Read, Seek},
};

use anyhow::{Context, Result, bail};
use binrw::{BinRead, NullString, binread};

/// RIFF chunk with [`Self::data`] as `Vec<u32>`.
#[binread]
#[derive(Debug)]
#[br(little)]
pub struct RiffChunkU32 {
    // temp because `data` stores its own length
    #[br(temp)]
    data_size: u32,

    #[br(try_calc = usize::try_from(data_size / 4), temp)]
    data_length: usize,

    /// The chunk data.
    #[br(count = data_length)]
    pub data: Vec<u32>,
    // no padding needed, data is inherently even (u32)
}

/// RIFF chunk with [`Self::data`] as [`Vec<u8>`].
#[binread]
#[derive(Debug)]
#[br(little)]
pub struct RiffChunkU8 {
    // size == length here since `data` is Vec<u8>
    #[br(temp)]
    data_size: u32,

    /// The chunk data.
    #[br(count = data_size, pad_after = data_size % 2)]
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
/// All frames must be in ICO format in order to store the required
/// cursor metadata (e.g, hotspot), so these are invalid flags:
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
    /// This is mainly for optimizing repeated frames.
    #[default]
    Unsequenced = 1,
    /// Contains ICO frames that play in the
    /// order they're defined (no "seq " chunk).
    Sequenced = 3,
}

/// Models an ANI file's header (or the "anih" chunk).
///
/// ```text
/// typedef struct {
///     DWORD cbSizeof;  // Should be sizeof(ANIHEADER) = 36 bytes.
///     DWORD cFrames;   // Number of frames in the frame list.
///     DWORD cSteps;    // Number of steps in the animation loop.
///
///     DWORD cx, cy;              // Not used.
///     DWORD cBitCount, cPlanes;  // Not used.
///
///     DWORD jifRate;  // Default display rate, in jiffies (1/60s).
///     DWORD fl;       // AF_ICON should be set. AF_SEQUENCE is optional.
/// } ANIHEADER;             
/// ```
#[binread]
#[derive(Debug, Default, PartialEq)]
pub struct AniHeader {
    #[br(temp)]
    anih_size: u32,
    #[br(assert(anih_size == header_size && header_size == 36), temp)]
    header_size: u32,
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

    // contains unused fields: cx, cy, cBitCount, cPlanes
    // spec says these should be zero/reserved, but windows
    // doesn't check--they render the cursor regardless
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
/// RIFF('ACON'  // ANI files have the "ACON" identifier.
///
///     [LIST('INFO'                   
///         [INAM(<ZSTR>)]  // Title.
///         [IART(<ZSTR>)]  // Author.
///     )]                             
///
///     'anih'(<ANIHEADER>)  // ANI file header.
///
///     // Rate table (array of jiffies). If the AF_SEQUENCE flag is set
///     // then the count is ANIHEADER.cSteps, otherwise ANIHEADER.cFrames.
///     ['rate'(<DWORD...>)]  
///
///     // Sequence table (array of frame index values). Should be present
///     // when the AF_SEQUENCE flag is set. Count is ANIHEADER.cSteps.
///     ['seq '(<DWORD...>)]  
///
///     LIST('fram'              // List of frames data. Count is ANIHEADER.cFrames.
///        'icon'(<icon_data_1>) // Frame 1
///        'icon'(<icon_data_2>) // Frame 2
///        ...                   // Frame 3..(ANIHEADER.cFrames)
///     )
/// )
/// ```
///
/// NOTE: The order shown here does not reflect how actual
///       ANI files can choose to order their fields.
///
/// - Brackets around a chunk indicate that it's optional.
/// - \<ZSTR\> indicates a null-terminated string.
#[derive(Default)]
pub struct AniFile {
    /// The header, i.e, the "anih" chunk.
    pub header: AniHeader,
    /// The title stored in the "INFO" ("LIST" subtype) chunk, with
    /// the identifier: "INAM". Note that this is rarely present.
    pub title: Option<NullString>,
    /// The author stored in the "INFO" ("LIST" subtype) chunk, with
    /// the identifier: "IART". Note that this is rarely present.
    pub author: Option<NullString>,
    /// Per-frame timings. Usually [`None`].
    ///
    /// ## Explanation
    ///
    /// rate:   `[t_0, t_1, t_2, ...]`\
    /// frames: `[f_0, f_1, f_2, ...]`
    ///
    /// Each frame, `f_n`, is displayed for `t_n`
    /// jiffies until `f_{n+1}` (modulo length).
    ///
    /// The rate is applied **after sequencing**, so `frames`
    /// is better said as the "display order", see [`Self::sequence`].
    pub rate: Option<RiffChunkU32>,
    /// Stores frame indices to indicate the order in which
    /// frames are played. Frames can also be repeated.
    ///
    /// ## Explanation
    ///
    /// frames:         `[f_0, f_1, f_2, f_3, ...]`\
    /// sequence:       `[2, 3, 0, 0, 1, ...]`\
    /// display order:  `[f_2, f_3, f_0, f_0, f_1, ...]`
    pub sequence: Option<RiffChunkU32>,
    /// ICO frames. Each frame should have a hotspot.
    ///
    /// Each ICO frame can contain multiple images, usually
    /// for supporting different sizes.
    ///
    /// _(although redundant, since Windows scales cursors already.)_
    pub ico_frames: Vec<RiffChunkU8>,
}

// skip ico_frames
impl fmt::Debug for AniFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AniFile")
            .field("header", &self.header)
            .field("title", &self.title)
            .field("author", &self.author)
            .field("rate", &self.rate)
            .field("sequence", &self.sequence)
            .finish_non_exhaustive()
    }
}

impl AniFile {
    /// Max blob size for any (dynamic length) chunk.
    const MAX_CHUNK_SIZE: usize = 2_097_152;

    /// Parses `ani_blob`.
    ///
    /// This is pretty complicated to parse (matches on fourcc)
    /// because of the "constraint" (or more like freedom?)
    /// of chunks being able to appearing in an arbitrary order.
    ///
    /// > [gdgsoft](https://www.gdgsoft.com/anituner/help/aniformat.htm):
    /// > "Any of the blocks ("ACON", "anih", "rate", or "seq ") can appear in any order."
    ///
    /// ## Errors
    ///
    /// Parsing is quite tricky. There's a load of errors that can happen:
    ///
    /// - overflow on calculations
    /// - duplicate chunks
    /// - missing required chunks (e.g, no [`AniHeader`])
    /// - blob lengths being unreasonably large (safety)
    /// - more complex invariants not being met, see [`Self::check_invariants`]
    pub fn from_blob(ani_blob: &[u8]) -> Result<Self> {
        if ani_blob.len() > Self::MAX_CHUNK_SIZE {
            bail!(
                "ani_blob.len()={} unreasonably large (2MB+)",
                ani_blob.len()
            )
        }

        // for sanity checks against read sizes
        let ani_blob_len_u64 = u64::try_from(ani_blob.len())?;
        let mut ani = Self::default();
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

            match &buf {
                b"LIST" => Self::parse_list(&mut cursor, &mut ani)?,
                b"anih" => {
                    if ani.header != AniHeader::default() {
                        bail!("duplicate 'anih' chunk");
                    }

                    ani.header =
                        AniHeader::read_le(&mut cursor).context("failed to read 'anih' chunk")?;
                }

                b"rate" => {
                    if ani.rate.is_some() {
                        bail!("duplicate 'rate' chunk");
                    }

                    ani.rate = Some(
                        RiffChunkU32::read_le(&mut cursor)
                            .context("failed to read 'rate' chunk")?,
                    );
                }

                b"seq " => {
                    if ani.sequence.is_some() {
                        bail!("duplicate 'seq ' chunk");
                    }

                    ani.sequence = Some(
                        RiffChunkU32::read_le(&mut cursor)
                            .context("failed to read 'seq ' chunk")?,
                    );
                }

                // consider attempting to read size and skipping
                // for unknown chunks (but it's a bit unreliable)
                _ => bail!("unexpected fourcc(?) buf={buf:?}"),
            }
        }

        Self::check_invariants(&ani)?;

        Ok(ani)
    }

    /// Helper for [`Self::from_blob`] for the "LIST" chunk.
    ///
    /// This can diverge depending on the subtype, which can
    /// either be "INFO" (title/author) or "fram" (frame data).
    ///
    /// The "INFO" chunk isn't required. The "fram" chunk is.
    fn parse_list(cursor: &mut Cursor<&[u8]>, ani: &mut Self) -> Result<()> {
        let ani_blob_size = cursor.get_ref().len();
        let mut buf = [0u8; 4];
        let mut list_id = [0u8; 4];
        cursor.read_exact(&mut buf)?; // list size
        cursor.read_exact(&mut list_id)?;
        let list_size = u32::from_le_bytes(buf);

        // excluding subtype fourcc (and padding)
        let list_data_size = list_size
            .checked_sub(4)
            .with_context(|| format!("underflow on list_size={list_size} - 4"))?;

        if usize::try_from(list_data_size)? > Self::MAX_CHUNK_SIZE {
            bail!("list_data_size={list_data_size} unreasonably large (2MB+)");
        }

        let end = cursor
            .position()
            .checked_add(u64::from(list_data_size))
            .with_context(|| {
                format!(
                    "overflow on cursor.position={} + list_data_size={list_data_size}",
                    cursor.position()
                )
            })?;

        // if we read `fram_size` bytes, are we still in the blob?
        if end > ani_blob_size.try_into()? {
            bail!("list_data_size={list_data_size} extends beyond blob");
        }

        match &list_id {
            b"INFO" => {
                while cursor.position() < end {
                    cursor.read_exact(&mut buf)?;

                    if buf == *b"INAM" {
                        if ani.title.is_some() {
                            bail!("duplicate 'INAM' subchunk in 'INFO'");
                        }

                        cursor.read_exact(&mut buf)?; // size
                        ani.title = Some(NullString::read_le(cursor)?);
                    } else if buf == *b"IART" {
                        if ani.author.is_some() {
                            bail!("duplicate 'IART' subchunk in 'INFO'");
                        }

                        cursor.read_exact(&mut buf)?; // size
                        ani.author = Some(NullString::read_le(cursor)?);
                    } else {
                        bail!("expected 'INAM' or 'IART' subchunk in 'INFO', instead got {buf:?}");
                    }
                }

                if list_data_size % 2 != 0 {
                    cursor.seek_relative(1)?;
                }
            }

            b"fram" => {
                if !ani.ico_frames.is_empty() {
                    bail!("duplicate 'fram' chunk");
                }

                let mut chunks = Vec::with_capacity(usize::try_from(ani.header.num_frames)?);

                while cursor.position() < end {
                    cursor.read_exact(&mut buf)?;

                    if buf != *b"icon" {
                        bail!("expected 'icon' subchunk, instead got {buf:?}");
                    }

                    let chunk = RiffChunkU8::read_le(cursor)
                        .context("failed to read 'icon' subchunk of 'fram'")?;

                    chunks.push(chunk);
                }

                if chunks.is_empty() {
                    bail!("failed to parse any frames from 'fram' chunk");
                }

                ani.ico_frames = chunks;
            }

            _ => bail!("unexpected list_id={list_id:?}"),
        }

        Ok(())
    }

    /// Helper function for checking invariants, since Clippy
    /// is complaining about my function body length :(
    ///
    /// Some checks produce warnings, while other produce errors.
    /// This is a deliberate choice, as Windows still renders
    /// files that the spec technically considers invalid.
    fn check_invariants(ani: &Self) -> Result<()> {
        use AniFlags::*;

        let hdr = &ani.header;
        let num_frames = usize::try_from(hdr.num_frames)?;
        let num_steps = usize::try_from(hdr.num_steps)?;

        if num_frames != ani.ico_frames.len() {
            bail!(
                "expected num_frames={num_frames}, instead got ico_frames.len()={}",
                ani.ico_frames.len()
            );
        }

        if hdr.jiffy_rate == 0 && ani.rate.is_none() && ani.ico_frames.len() > 1 {
            bail!("no frame timings (>1 frames): jiffy_rate=0, ani.rate=None");
        }

        if let Some(seq) = &ani.sequence
            && seq.data.iter().max() >= Some(&hdr.num_frames)
        {
            bail!("frame indices of 'seq ' chunk go out of bounds");
        }
        if let Some(seq) = &ani.sequence
            && hdr.flags == Unsequenced
            && seq.data != (0..hdr.num_steps).collect::<Vec<_>>()
        {
            eprintln!(
                "[warning] expected 'seq ' chunk to be None from flags={:?}, found \
                non-linear sequence={:?}. note that this sequence will still be used",
                hdr.flags, ani.sequence
            );
        }

        // rate maps to sequenced frames
        if let Some(rate) = &ani.rate
            && rate.data.len() != num_steps
        {
            bail!(
                "expected num_steps={num_steps}, instead got rate.len()={}",
                rate.data.len(),
            )
        }

        if hdr.flags == Sequenced && ani.sequence.is_none() {
            eprintln!(
                "[warning] expected 'seq ' chunk from flags={:?}, found None",
                hdr.flags
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::fmt::Write;

    use super::*;
    use crate::from_root;

    /// Parses a file and checks everything matches expected results.
    // (sort of lazy but it's better than nothing)
    #[test]
    fn good_ani() {
        const ANI_BLOB: &[u8] = include_bytes!(from_root!("/testing/fixtures/neuro_alt.ani"));
        const ANI_FRAMES: &str = include_str!(from_root!("/testing/fixtures/neuro_alt_frames"));

        const {
            assert!(
                size_of::<AniFile>() == 136,
                "AniFile fields have changed, update tests and this number accordingly"
            );
        }

        let ani = AniFile::from_blob(ANI_BLOB).unwrap();
        let hdr = &ani.header;

        assert_eq!(hdr.num_frames, 10);
        assert_eq!(hdr.num_steps, 21);
        assert_eq!(hdr.jiffy_rate, 6);
        assert_eq!(hdr.flags, AniFlags::Sequenced);

        assert!(ani.rate.is_none());

        assert_eq!(
            ani.sequence.as_ref().unwrap().data,
            &[
                0, 1, 2, 2, 3, 3, 3, 3, 4, 5, 6, 7, 3, 3, 3, 2, 2, 2, 3, 8, 9
            ]
        );

        assert_eq!(
            usize::try_from(hdr.num_frames).unwrap(),
            ani.ico_frames.len()
        );

        assert_eq!(
            usize::try_from(hdr.num_steps).unwrap(),
            ani.sequence.as_ref().unwrap().data.len()
        );

        let mut ani_frames = String::new();

        for frame in ani.ico_frames {
            writeln!(&mut ani_frames, "{:?}", frame.data).unwrap();
        }

        assert_eq!(ani_frames, ANI_FRAMES);
    }
}
