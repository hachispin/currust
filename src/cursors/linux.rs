//! Contains structs that model byte layouts of
//! the [Xcursor](https://man.archlinux.org/man/Xcursor.3) format.
//!
//! The Xcursor format generally follows this structure:
//!
//! 1) File header that contains a list of [`TableOfContents`]
//! 2) Each [`TableOfContents`] describes a chunk.
//! 3) Each chunk describes either an image or a comment*.
//! 4) Image chunks contain packed ARGB pixels.
//!
//! * Modeling for comment chunks is left out on purpose.

#![allow(unused)] // REMOVE ME LATER

use std::{fs::File, io::Cursor, path::Path};

use binrw::BinWrite;
use log::warn;
use miette::{Context, IntoDiagnostic, Result};

use super::common::CursorImage;

/// Converts the bytes in `rgba` to ARGB format in-place.
///
/// ## Panics
///
/// Panics if the length of `rgba` is not a multiple of four.
pub fn to_argb(rgba: &mut [u8]) {
    assert!(
        rgba.len().is_multiple_of(4),
        "invalid RGBA, each pixel should have 4 channels"
    );

    for pixel in rgba.chunks_exact_mut(4) {
        pixel.swap(0, 3); // AGBR
        pixel.swap(1, 2); // ABGR
        pixel.swap(1, 3); // ARGB
    }
}

#[derive(BinWrite)]
#[bw(little, magic = b"Xcur", assert(*num_toc as usize == tocs.len()))]
struct XcursorHeader {
    header_size: u32,
    version: u32,
    num_toc: u32,
    tocs: Vec<TableOfContents>,
}

/// The fields here are specific to image chunks,
/// since I don't plan on adding comment chunk support.
///
/// Only the [`Self::position`] field is useful here,
/// everything else is redundant/duplicated in the chunk.
#[derive(BinWrite)]
#[bw(little, assert(*type_id == TableOfContents::IMG_TYPE_ID))]
struct TableOfContents {
    type_id: u32,
    /// Also known as the subtype.
    _nominal_size: u32,
    position: u32,
}

// For constants
impl TableOfContents {
    /// Value used to indicate a chunk is an image chunk.
    const IMG_TYPE_ID: u32 = 0xfffd_0002;

    /// Header size for image chunks.
    const IMG_HEADER_SIZE: u32 = 36;

    const VERSION: u32 = 1;
}

/// Prefer using the fields here over the ones stored in [`TableOfContents`].
///
/// Use [`Self::new`] to handle invariant fields.
#[derive(BinWrite)]
#[bw(
    little,
    assert(*header_size == TableOfContents::IMG_HEADER_SIZE),
    assert(*type_id == TableOfContents::IMG_TYPE_ID)
)]
struct ImageChunkHeader {
    header_size: u32,
    type_id: u32,
    /// Also known as the subtype.
    nominal_size: u32,
    version: u32,
    width: u32,
    height: u32,
    hotspot_x: u32,
    hotspot_y: u32,
    delay: u32,
}

impl From<&CursorImage> for ImageChunkHeader {
    fn from(cursor: &CursorImage) -> Self {
        Self {
            header_size: TableOfContents::IMG_HEADER_SIZE,
            type_id: TableOfContents::IMG_TYPE_ID,
            nominal_size: cursor.height.max(cursor.width),
            version: TableOfContents::VERSION,
            width: cursor.width,
            height: cursor.height,
            hotspot_x: cursor.hotspot_x,
            hotspot_y: cursor.hotspot_y,
            delay: 0, // Only matters in animated cursors
        }
    }
}

#[derive(BinWrite)]
#[bw(little, assert(argb.len() == (header.width * header.height * 4) as usize))]
struct ImageChunk {
    header: ImageChunkHeader,
    argb: Vec<u8>,
}

impl From<&CursorImage> for ImageChunk {
    fn from(cursor: &CursorImage) -> Self {
        let header = ImageChunkHeader::from(cursor);
        let mut argb = cursor.rgba.clone();
        to_argb(&mut argb);

        Self { header, argb }
    }
}

/// Represents the structure of an Xcursor file.
#[derive(BinWrite)]
pub struct Xcursor {
    header: XcursorHeader,
    chunks: Vec<ImageChunk>,
}

impl Xcursor {
    /// Conversion function between generic cursors and Xcursor.
    ///
    /// ## Panics
    ///
    /// If `cursor` is empty.
    #[must_use]
    pub fn from_cursor_image(cursor: &[CursorImage]) -> Self {
        assert!(!cursor.is_empty(), "`cursor` cannot be empty");

        if cursor.len() > 1 {
            warn!("Unstable feature: creating `Xcursor` from multiple cursor images");
        }

        let cursor = cursor.first().unwrap();
        let chunk = ImageChunk::from(cursor);

        let toc_pos = 40u32;

        #[allow(clippy::cast_possible_truncation)]
        let toc = TableOfContents {
            type_id: chunk.header.type_id,
            _nominal_size: chunk.header.nominal_size,
            position: toc_pos,
        };

        let header = XcursorHeader {
            header_size: 24,
            version: 1,
            num_toc: 1,
            tocs: vec![toc],
        };

        Self {
            header,
            chunks: vec![chunk],
        }
    }

    /// Saves the current Xcursor the `path`.
    ///
    /// ## Errors
    ///
    /// If `path` can't be created.
    pub fn save(&self, path: &Path) -> Result<()> {
        let mut file = File::create(path)
            .into_diagnostic()
            .with_context(|| format!("Failed to create file {}", path.display()))?;

        self.write_le(&mut file);

        Ok(())
    }
}
