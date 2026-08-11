//! Module for parsing [ANI](https://en.wikipedia.org/wiki/ANI_(file_format)).
//!
//! Note that "CUR" and "ICO" are used interchangeably, since
//! the only notable difference is the presence of a hotspot.
//!
//! You may find it helpful to also read about [RIFF](https://en.wikipedia.org/wiki/Resource_Interchange_File_Format).

use std::{
    fmt,
    io::{Cursor, Read, Seek, SeekFrom},
    range::Range,
};

use anyhow::{Result, anyhow, bail};
use binrw::{BinRead, binread};

use crate::warn;

/// Generic RIFF chunk structure.
#[binread]
#[derive(Debug)]
#[br(little)]
#[br(stream = s)]
pub struct RiffChunk {
    id: [u8; 4],
    size: u32,

    // Just for calculations.
    #[br(temp, try_calc = s.stream_position())]
    start: u64,
    #[br(temp, try_calc = start.checked_add(u64::from(size)).ok_or_else(|| {
        anyhow!("overflow when calculating RiffChunk end for id={id:?}")
    }))]
    end: u64,

    /// Range of data, excludes padding.
    #[br(calc = (start..end).into())]
    data: Range<u64>,

    /// Where the next chunk should start and another [`RiffChunk`] can be read.
    #[br(try_calc = end.checked_add(u64::from(size) & 1).ok_or_else(|| {
        anyhow!("overflow when calculating RiffChunk next for id={id:?}")
    }))]
    next: u64,
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
    #[br(assert(header_size == 36), temp)]
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
    pub title: Option<String>,
    /// The author stored in the "INFO" ("LIST" subtype) chunk, with
    /// the identifier: "IART". Note that this is rarely present.
    pub author: Option<String>,
    /// Per-frame timings. Usually [`None`].
    ///
    /// rate:   `[t_0, t_1, t_2, ...]`\
    /// frames: `[f_0, f_1, f_2, ...]`
    ///
    /// Each frame, `f_n`, is displayed for `t_n` jiffies until `f_{n+1}` (modulo length).
    ///
    /// The rate is applied **after sequencing**, so `frames` is
    /// better said as the "display order", see [`Self::sequence`].
    pub rate: Option<Vec<u32>>,
    /// Stores frame indices to indicate the order in which
    /// frames are played. Frames can also be repeated.
    ///
    /// frames:         `[f_0, f_1, f_2, f_3, ...]`\
    /// sequence:       `[2, 3, 0, 0, 1, ...]`\
    /// display order:  `[f_2, f_3, f_0, f_0, f_1, ...]`
    pub sequence: Option<Vec<u32>>,
    /// ICO frames. Each frame should have a hotspot.
    ///
    /// Each ICO frame can contain multiple images, usually for supporting different sizes.
    ///
    /// _Although redundant, since Windows scales cursors already._
    pub ico_frames: Vec<Vec<u8>>,
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

#[derive(Default)]
struct AniParserState {
    header: Option<Range<u64>>,
    title: Option<Range<u64>>,
    author: Option<Range<u64>>,
    rate: Option<Range<u64>>,
    sequence: Option<Range<u64>>,
    ico_frames: Option<Range<u64>>,
}

fn process_ranges(blob: &[u8], state: &AniParserState) -> Result<AniFile> {
    // helpers

    let to_usize_range = |range: Range<u64>| -> Result<_> {
        let start = usize::try_from(range.start)?;
        let end = usize::try_from(range.end)?;

        Ok(start..end)
    };

    let bytes_to_string = |r: Range<u64>| {
        let string = &blob[to_usize_range(r)?];

        let string = if let Some(s) = string.strip_suffix(b"\0") {
            s
        } else {
            warn!("'INFO' string is not null-terminated");
            string
        };

        str::from_utf8(string)
            .map(ToString::to_string)
            .map_err(Into::<anyhow::Error>::into)
    };

    let to_u32_vec = |r: Range<u64>| {
        let bytes = &blob[to_usize_range(r)?];

        let (bytes, rem) = bytes.as_chunks::<4>();

        if !rem.is_empty() {
            bail!("u32 data not divisible by 4")
        }

        anyhow::Ok(
            bytes
                .iter()
                .map(|&b| u32::from_le_bytes(b))
                .collect::<Vec<_>>(),
        )
    };

    // required stuff

    let Some(header) = state.header else {
        bail!("'anih' chunk is required but is missing")
    };

    let Some(ico_frames) = state.ico_frames else {
        bail!("'fram' chunk is required but is missing")
    };

    let header = AniHeader::read(&mut Cursor::new(&blob[to_usize_range(header)?]))?;
    let fram = &blob[to_usize_range(ico_frames)?];
    let mut cursor = Cursor::new(fram);
    let mut ico_frames = Vec::with_capacity(usize::try_from(header.num_frames)?);

    while cursor.position() < u64::try_from(fram.len())? {
        let icon = RiffChunk::read(&mut cursor)?;

        if icon.id != *b"icon" {
            bail!("expected 'icon' subchunks, instead got {:?}", icon.id);
        }

        let mut bytes = vec![0; usize::try_from(icon.size)?];
        cursor.read_exact(&mut bytes)?;
        ico_frames.push(bytes);

        cursor.seek(SeekFrom::Start(icon.next))?;
    }

    // optional things

    let title = state.title.map(bytes_to_string).transpose()?;
    let author = state.author.map(bytes_to_string).transpose()?;
    let rate = state.rate.map(to_u32_vec).transpose()?;
    let sequence = state.sequence.map(to_u32_vec).transpose()?;

    Ok(AniFile {
        header,
        title,
        author,
        rate,
        sequence,
        ico_frames,
    })
}

impl AniFile {
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
        let ani_blob_len_u64 = u64::try_from(ani_blob.len())?;
        let mut state = AniParserState::default();
        let mut cursor = Cursor::new(ani_blob);

        let riff = RiffChunk::read(&mut cursor)?;

        if riff.id != *b"RIFF" {
            bail!("expected 'RIFF' chunk, instead got {:?}", riff.id);
        }

        // NOTE: stricter checks like this fail on "valid" files
        // `riff_size == blob.len() - 8`
        // https://github.com/quantum5/win2xcur/commit/ac9552ce83d2955a96a4d7a5cfde7c113ec5a4c5
        if u64::from(riff.size) > ani_blob_len_u64 {
            bail!("riff_size={} extends beyond blob", riff.size)
        }

        let mut list_type = [0_u8; 4];
        cursor.read_exact(&mut list_type)?;

        if list_type != *b"ACON" {
            bail!("expected 'ACON' as 'RIFF' subtype, instead got {list_type:?}");
        }

        // read chunks and parse
        while cursor.position() < ani_blob.len().try_into()? {
            let chunk = RiffChunk::read(&mut cursor)?;

            match &chunk.id {
                b"LIST" => Self::parse_list(&mut cursor, &mut state, &chunk)?,
                b"anih" => {
                    if state.header.is_some() {
                        bail!("read duplicate 'anih' chunk at {}", cursor.position());
                    }

                    state.header = Some(chunk.data);
                }

                b"rate" => {
                    if state.rate.is_some() {
                        bail!("read duplicate 'rate' chunk at {}", cursor.position());
                    }

                    state.rate = Some(chunk.data);
                }

                b"seq " => {
                    if state.sequence.is_some() {
                        bail!("read duplicate 'seq ' chunk at {}", cursor.position());
                    }

                    state.sequence = Some(chunk.data);
                }

                _ => (),
            }

            cursor.seek(SeekFrom::Start(chunk.next))?;
        }

        let ani = process_ranges(ani_blob, &state)?;
        Self::check_invariants(&ani)?;

        Ok(ani)
    }

    /// Helper for [`Self::from_blob`] for the "LIST" chunk.
    ///
    /// This can diverge depending on the subtype, which can
    /// either be "INFO" (title/author) or "fram" (frame data).
    ///
    /// The "INFO" chunk isn't required. The "fram" chunk is.
    fn parse_list(
        cursor: &mut Cursor<&[u8]>,
        state: &mut AniParserState,
        list_chunk: &RiffChunk,
    ) -> Result<()> {
        let end = cursor
            .position()
            .checked_add(u64::from(list_chunk.size))
            .ok_or_else(|| anyhow!("overflow when calculating end of list chunk"))?;

        let mut list_type = [0_u8; 4];
        cursor.read_exact(&mut list_type)?;

        match &list_type {
            b"INFO" => {
                while cursor.position() < end {
                    let subchunk = RiffChunk::read(cursor)?;

                    // Let INFO be overridden as it's non-essential.
                    if subchunk.id == *b"INAM" {
                        state.title = Some(subchunk.data);
                    } else if subchunk.id == *b"IART" {
                        state.author = Some(subchunk.data);
                    }

                    cursor.seek(SeekFrom::Start(subchunk.next))?;
                }
            }

            b"fram" => {
                if state.ico_frames.is_some() {
                    bail!("read duplicate 'fram' chunk at {}", cursor.position());
                }

                // exclude list type (fram)
                state.ico_frames = Some((cursor.position()..end).into());
            }

            _ => (),
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
            && rate.len() != num_steps
        {
            bail!(
                "expected num_steps={num_steps}, instead got rate.len()={}",
                rate.len(),
            )
        }

        if hdr.jiffy_rate == 0 && ani.rate.is_none() && ani.ico_frames.len() > 1 {
            bail!("no frame timings (>1 frames): jiffy_rate=0, ani.rate=None");
        }

        if let Some(seq) = &ani.sequence
            && seq.iter().max() >= Some(&hdr.num_frames)
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
            && !seq.iter().copied().eq(0..hdr.num_steps)
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

        let ani = AniFile::from_blob(ANI_BLOB).unwrap();
        let hdr = &ani.header;

        assert_eq!(hdr.num_frames, 10);
        assert_eq!(hdr.num_steps, 21);
        assert_eq!(hdr.jiffy_rate, 6);
        assert_eq!(hdr.flags, AniFlags::Sequenced);

        assert!(ani.rate.is_none());

        assert_eq!(
            ani.sequence.as_ref().unwrap(),
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
            ani.sequence.as_ref().unwrap().len()
        );

        let mut ani_frames = String::new();

        for frame in ani.ico_frames {
            writeln!(&mut ani_frames, "{frame:?}").unwrap();
        }

        assert_eq!(ani_frames, ANI_FRAMES);
    }
}
