//! Module for parsing [ANI](https://en.wikipedia.org/wiki/ANI_(file_format)).
//!
//! Note that "CUR" and "ICO" are used interchangeably, since
//! the only notable difference is the presence of a hotspot.
//!
//! You may find it helpful to also read about [RIFF](https://en.wikipedia.org/wiki/Resource_Interchange_File_Format).

use std::{
    fmt,
    io::{Cursor, Read},
};

use anyhow::{Context, Result, bail};
use binrw::{BinRead, NullString, binread};

use crate::warn;

/// RIFF chunk with [`Self::data`] as [`Vec<u32>`].
#[binread]
#[derive(Debug)]
#[br(little)]
pub struct RiffChunkU32 {
    // temp because `data` stores its own length
    #[br(temp)]
    data_size: u32,

    #[br(try_calc = usize::try_from(data_size / 4), temp)]
    data_length: usize,

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

    #[br(count = data_size, pad_after = data_size % 2)]
    pub data: Vec<u8>,
    // padding byte skipped with `pad_after`
}

// NOTE: this is storing the valid combinations of bitflags and are not meant to be composable.

/// Contains possible flag combinations for [`AniHeader`], which describe the "seq " chunk.
///
/// The ANI format defines these flags:
///
/// ```text
/// #define AF_ICON 0x1         // Frames are in Windows ICO format.
/// #define AF_SEQUENCE 0x2     // Animation is sequenced.
/// ```
///
/// All of the frames must be in ICO format in order to store the
/// required cursor metadata (e.g., hotspot), so these are invalid flags:
///
/// - `0`: no flags are set
/// - `2`: frames are not ICO
#[derive(Debug, PartialEq, BinRead)]
#[br(little)]
#[br(repr = u32)]
enum AniFlags {
    /// Contains ICO frames that play in the order they're defined (no "seq " chunk).
    Unsequenced = 1,
    /// Contains ICO frames with a custom "seq " chunk,
    /// which defines the order frames should be played.
    ///
    /// This is mainly for optimizing repeated frames.
    Sequenced = 3,
}

/// Models an ANI file's header (or the "anih" chunk).
#[binread]
#[derive(Debug, PartialEq)]
#[br(little)]
pub struct AniHeader {
    #[br(temp)]
    anih_size: u32,
    #[br(assert(anih_size == header_size && header_size == 36), temp)]
    header_size: u32,
    /// Number of frames in "fram" LIST. Not to be confused with [`Self::num_steps`]:
    ///
    /// ```text
    /// sequence = [0, 1, 2, 1] => num_steps  = 4
    /// frames   = [0, 1, 2]    => num_frames = 3
    /// ```
    pub num_frames: u32,
    /// Number of steps in the animation loop. Not to be confused with [`Self::num_frames`]:
    ///
    /// ```text
    /// sequence = [0, 1, 2, 1] => num_steps  = 4
    /// frames   = [0, 1, 2]    => num_frames = 3
    /// ```

    // This padding contains the unused fields: `cx`, `cy`, `cBitCount`, `cPlanes`. Spec says these
    // should be zero/reserved, but Windows doesn't check--the cursor is rendered regardless.
    #[br(pad_after = 16)]
    pub num_steps: u32,

    /// Default jiffy rate if "rate" isn't provided.
    pub jiffy_rate: u32,
    // Flags to indicate whether the "seq " chunk exists.
    flags: AniFlags,
}

/// Models a parsed ANI file.
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
    /// rate:   `[t_0, t_1, t_2, ...]`\
    /// frames: `[f_0, f_1, f_2, ...]`
    ///
    /// Each frame, `f_n`, is displayed for `t_n` jiffies until `f_{n+1}` (modulo length).
    ///
    /// The rate is applied **after sequencing**, so `frames` is
    /// better said as the "display order", see [`Self::sequence`].
    pub rate: Option<RiffChunkU32>,
    /// Stores frame indices to indicate the order in which
    /// frames are played. Frames can also be repeated.
    ///
    /// frames:         `[f_0, f_1, f_2, f_3, ...]`\
    /// sequence:       `[2, 3, 0, 0, 1, ...]`\
    /// display order:  `[f_2, f_3, f_0, f_0, f_1, ...]`
    pub sequence: Option<RiffChunkU32>,
    /// ICO frames. Each frame should have a hotspot.
    ///
    /// Each ICO frame can contain multiple images, usually for supporting different sizes.
    ///
    /// _Although redundant, since Windows scales cursors already._
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

/// For tracking duplicates.
#[derive(Default)]
struct AniParserState {
    pub header: Option<AniHeader>,
    pub title: Option<NullString>,
    pub author: Option<NullString>,
    pub rate: Option<RiffChunkU32>,
    pub sequence: Option<RiffChunkU32>,
    pub ico_frames: Option<Vec<RiffChunkU8>>,
}

impl TryFrom<AniParserState> for AniFile {
    type Error = anyhow::Error;

    fn try_from(state: AniParserState) -> Result<Self> {
        let Some(header) = state.header else {
            bail!("AniHeader is required")
        };

        let Some(ico_frames) = state.ico_frames else {
            bail!("ico_frames is required")
        };

        Ok(Self {
            header,
            title: state.title,
            author: state.author,
            rate: state.rate,
            sequence: state.sequence,
            ico_frames,
        })
    }
}

impl AniFile {
    /// Max blob size for any (dynamic length) chunk.
    const MAX_CHUNK_SIZE: usize = 2_097_152;

    /// Parses `ani_blob`.
    ///
    /// ## Errors
    ///
    /// Parsing is quite tricky. Some errors that can happen are:
    ///
    /// - {over,under}flow on calculations
    /// - duplicate chunks
    /// - missing required chunks (e.g, no [`AniHeader`])
    /// - more complex invariants not being met, see [`Self::check_invariants`]
    ///
    /// ## References
    ///
    /// - [gdgsoft](https://www.gdgsoft.com/anituner/help/aniformat.htm)
    pub fn from_blob(ani_blob: &[u8]) -> Result<Self> {
        // for sanity checks against read sizes
        let ani_blob_len_u64 = u64::try_from(ani_blob.len())?;
        let mut state = AniParserState::default();
        let mut cursor = Cursor::new(ani_blob);
        let mut buf = [0_u8; 4];
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
                b"LIST" => Self::parse_list(&mut cursor, &mut state)?,
                b"anih" => {
                    if state.header.is_some() {
                        bail!("duplicate 'anih' chunk");
                    }

                    state.header = Some(
                        AniHeader::read_le(&mut cursor).context("failed to read 'anih' chunk")?,
                    );
                }

                b"rate" => {
                    if state.rate.is_some() {
                        bail!("duplicate 'rate' chunk");
                    }

                    state.rate = Some(
                        RiffChunkU32::read_le(&mut cursor)
                            .context("failed to read 'rate' chunk")?,
                    );
                }

                b"seq " => {
                    if state.sequence.is_some() {
                        bail!("duplicate 'seq ' chunk");
                    }

                    state.sequence = Some(
                        RiffChunkU32::read_le(&mut cursor)
                            .context("failed to read 'seq ' chunk")?,
                    );
                }

                // consider attempting to read size and skipping
                // for unknown chunks (but it's a bit unreliable)
                _ => bail!("unexpected fourcc(?) buf={buf:?}"),
            }
        }

        let ani = state.try_into()?;

        Self::check_invariants(&ani)?;

        Ok(ani)
    }

    /// Helper for [`Self::from_blob`] for the "LIST" chunk.
    ///
    /// This can diverge depending on the subtype, which can
    /// either be "INFO" (title/author) or "fram" (frame data).
    ///
    /// The "INFO" chunk isn't required. The "fram" chunk is.
    fn parse_list(cursor: &mut Cursor<&[u8]>, state: &mut AniParserState) -> Result<()> {
        let ani_blob_size = cursor.get_ref().len();
        let mut buf = [0_u8; 4];
        let mut list_id = [0_u8; 4];
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

        if end > ani_blob_size.try_into()? {
            bail!("list_data_size={list_data_size} extends beyond blob");
        }

        match &list_id {
            b"INFO" => {
                while cursor.position() < end {
                    cursor.read_exact(&mut buf)?;

                    let field = if buf == *b"INAM" {
                        &mut state.title
                    } else if buf == *b"IART" {
                        &mut state.author
                    } else {
                        bail!("expected 'INAM' or 'IART' subchunk in 'INFO', instead got {buf:?}");
                    };

                    if field.is_some() {
                        bail!("duplicate 'INAM' or 'IART' subchunk in 'INFO'");
                    }

                    // size of string
                    cursor.read_exact(&mut buf)?;
                    *field = Some(NullString::read_le(cursor)?);
                }
            }

            b"fram" => {
                if state.ico_frames.is_some() {
                    bail!("duplicate 'fram' chunk");
                }

                let mut chunks = Vec::new();

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

                state.ico_frames = Some(chunks);
            }

            _ => bail!("unexpected list_id={list_id:?}"),
        }

        Ok(())
    }

    /// Helper function for checking invariants, since Clippy
    /// is complaining about my function body length :(
    ///
    /// Some checks produce warnings, while other produce errors. This is a deliberate
    /// choice, as Windows still renders files that the spec technically considers invalid.
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

        if let Some(rate) = &ani.rate
            && rate.data.len() != num_steps
        {
            bail!(
                "expected num_steps={num_steps}, instead got rate.len()={}",
                rate.data.len(),
            )
        }

        if hdr.jiffy_rate == 0 && ani.rate.is_none() && ani.ico_frames.len() > 1 {
            bail!("no frame timings (>1 frames): jiffy_rate=0, ani.rate=None");
        }

        if let Some(seq) = &ani.sequence
            && seq.data.iter().max() >= Some(&hdr.num_frames)
        {
            bail!("frame indices of 'seq ' chunk go out of bounds");
        }

        if hdr.flags == Sequenced && ani.sequence.is_none() {
            warn!(
                "expected 'seq ' chunk from flags={:?}, found None. the \
                order in which frames were stored will be used instead",
                hdr.flags
            );
        }

        if let Some(seq) = &ani.sequence
            && hdr.flags == Unsequenced
            && seq.data.iter().copied().eq(0..hdr.num_steps)
        {
            warn!(
                "expected 'seq ' chunk to be None from flags={:?}, found the non \
                linear sequence={:?}, note that this sequence will still be used",
                hdr.flags, ani.sequence
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Write;

    use super::*;
    use crate::from_root;

    /// Parses a file and checks everything matches expected results.
    // (sort of lazy but it's better than nothing)
    #[test]
    fn good_ani() {
        const ANI_FRAMES: &str = include_str!(from_root!("/testing/fixtures/neuro_alt_frames"));
        const ANI_BLOB: &[u8] = include_bytes!(from_root!("/testing/fixtures/neuro/Neuro alt.ani"));

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
