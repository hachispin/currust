//! Contains the [`GenericCursor`] struct.
//!
//! This represents a full static/animated cursor.

use crate::formats::{ani::AniFile, xcursor::Xcursor};

use super::{
    cursor_image::{CursorImage, CursorImages},
    cursor_role::CursorRole,
};

use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{BufWriter, Cursor, Write},
    path::Path,
};

use anyhow::{Context, Result, bail};
use binrw::BinWrite;
use fast_image_resize::ResizeAlg;
use ico::IconDir;

/// Represents a generic cursor.
#[derive(Debug)]
pub struct GenericCursor {
    /// Source images and scaled images as (`scale_factor`, `CursorImages`) pairs.
    ///
    /// The canonical base (with `scale_factor = 1.0`) is chosen with the logic in
    /// [`Self::get_base_dimensions`]. This prevents the situation where the source already contains
    /// multiple scaled variants (e.g., 32x32, 64x64), with `--scale-to 2.0` scaling each variant,
    /// giving you a cursor with 32x32, 64x64, 128x128 sizes (which is probably not expected).
    images: Vec<(f64, CursorImages)>,
    /// The cursor's role when used in a theme.
    pub role: Option<CursorRole>,
    /// Info stored within the cursor.
    ///
    /// For ANI this is its "INFO" chunk, while for Xcursor this is a comment chunk.
    info: Option<String>,
}

impl GenericCursor {
    fn from_source(
        base: CursorImages,
        variants: Vec<CursorImages>,
        info: Option<String>,
    ) -> Result<Self> {
        let base_nominal = f64::from(base.first().nominal_size());
        let base_len = base.len();
        let mut images = Vec::with_capacity(variants.len() + 1);
        images.push((1.0, base));

        for variant in variants {
            if variant.len() != base_len {
                bail!("expected {base_len} frames per size, got {}", variant.len());
            }

            let scale_factor = f64::from(variant.first().nominal_size()) / base_nominal;

            if images.iter().any(|(scale, _)| *scale == scale_factor) {
                bail!("duplicate source cursor scale factor: {scale_factor}");
            }

            images.push((scale_factor, variant));
        }

        Ok(Self {
            images,
            role: None,
            info,
        })
    }

    /// Adds a variant scaled from the canonical base, if the scale is new.
    ///
    /// ## Errors
    ///
    /// If resizing fails.
    pub fn add_scale(&mut self, scale_factor: f64, algorithm: ResizeAlg) -> Result<()> {
        // some cursors already store scaled versions
        if self.images.iter().any(|(scale, _)| *scale == scale_factor) {
            eprintln!("scale_factor={scale_factor} already added, skipping");
            return Ok(());
        }

        let scaled_images: Vec<CursorImage> = self.images[0]
            .1
            .inner()
            .iter()
            .map(|c| c.scaled_to(scale_factor, algorithm))
            .collect::<Result<_>>()?;

        self.images.push((scale_factor, scaled_images.try_into()?));

        Ok(())
    }

    /// Reads the file and parses based on extension.
    ///
    /// ## Errors
    ///
    /// If `path` has no extension or an extension that isn't "ani" or "cur".
    pub fn from_path<P: AsRef<Path>>(path: P, base_nominal_size: Option<u32>) -> Result<Self> {
        let path = path.as_ref();

        let Some(ext) = path.extension() else {
            bail!("path has no extension; expected 'cur' or 'ani'")
        };

        let ext = ext.to_ascii_lowercase();

        let cursor = if ext == "cur" {
            Self::from_cur_path(path, base_nominal_size)
        } else if ext == "ani" {
            Self::from_ani_path(path, base_nominal_size)
        } else {
            bail!(
                "expected extension 'cur' or 'ani' for path, got ext={}",
                ext.display()
            )
        }?;

        Ok(cursor)
    }

    /// Reads and parses a cursor from `cur_path`, which
    /// must be a path to a Windows cursor file (i.e, CUR).
    ///
    /// ## Errors
    ///
    /// If a file handle to `cur_path` can't be opened,
    /// or the file stored is not a CUR file.
    pub fn from_cur_path<P: AsRef<Path>>(
        cur_path: P,
        base_nominal_size: Option<u32>,
    ) -> Result<Self> {
        let cur_path = cur_path.as_ref();
        let handle = fs::read(cur_path).context("(filesystem) failed to read")?;
        let icon_dir = IconDir::read(Cursor::new(handle)).context("failed to read `IconDir`")?;
        let entries = icon_dir.entries();

        if entries.is_empty() {
            bail!("no stored images found");
        }

        let mut base = Vec::new();
        let mut scaled = Vec::new();
        let base_dims = Self::get_base_dimensions(&[&icon_dir]);

        for entry in entries {
            let image = CursorImage::from_entry(entry, base_nominal_size, 0)?;

            if image.dimensions() == base_dims {
                base.push(image);
            } else {
                scaled.push(vec![image].try_into()?);
            }
        }

        Self::from_source(base.try_into()?, scaled, None)
    }

    /// Parses `ani_path`.
    ///
    /// ## Errors
    ///
    /// - `ani_path` fails to be parsed as an [`IconDir`]
    /// - Stored RGBA in ICO frames fail to be decoded.
    /// - Frames are inconsistent, see [`CursorImages`].
    /// - [`TryInto`] conversions fail (between primitive types).
    pub fn from_ani_path<P: AsRef<Path>>(
        ani_path: P,
        base_nominal_size: Option<u32>,
    ) -> Result<Self> {
        let ani_blob = fs::read(&ani_path)?;
        let ani_file = AniFile::from_blob(&ani_blob)?;
        let header = &ani_file.header;

        // optional info stuff
        let info = match (ani_file.title, ani_file.author) {
            (Some(t), Some(a)) => Some(format!("'{t}' by '{a}'")),
            (Some(t), None) => Some(format!("'{t}' by [unknown]")),
            (None, Some(a)) => Some(format!("[unknown] by '{a}'")),
            (None, None) => None,
        };

        // read each ico frame
        let icos: Vec<IconDir> = ani_file
            .ico_frames
            .into_iter()
            .map(|chunk| {
                let ico = IconDir::read(&mut Cursor::new(&chunk))?;

                if ico.entries().is_empty() {
                    bail!("ANI frame contains no images");
                }

                Ok(ico)
            })
            .collect::<Result<_>>()?;

        // get display order as indices into icos
        let sequence: Option<Vec<usize>> = ani_file
            .sequence
            .map(|chunk| chunk.into_iter().map(usize::try_from).collect())
            .transpose()?;

        // indices validated in-bounds in AniFile
        let sequenced_icos: Vec<&IconDir> = sequence.map_or_else(
            || icos.iter().collect(),
            |v| v.into_iter().map(|idx| &icos[idx]).collect(),
        );

        // use default timings in header, or custom one if defined
        let num_steps = usize::try_from(header.num_steps)?;
        let delays_jiffies = ani_file
            .rate
            .unwrap_or_else(|| vec![header.jiffy_rate; num_steps]);

        // jiffies are 1/60th of a second
        //
        // NOTE: this might cause slight diffs compared
        //       to other converters because of rounding
        let delays_ms: Vec<u32> = delays_jiffies
            .into_iter()
            .map(|j| (j * 1000 + 30) / 60) // round by adding 30
            .collect();

        let base_dims = Self::get_base_dimensions(&sequenced_icos);
        let mut by_dimensions = BTreeMap::<_, Vec<_>>::new();

        for (ico, delay) in sequenced_icos.iter().zip(delays_ms) {
            for entry in ico.entries() {
                let image = CursorImage::from_entry(entry, base_nominal_size, delay)?;
                by_dimensions
                    .entry(image.dimensions())
                    .or_default()
                    .push(image);
            }
        }

        let base = by_dimensions
            .remove(&base_dims)
            .context("no base frames found")?;
        let variants = by_dimensions
            .into_values()
            .map(CursorImages::try_from)
            .collect::<Result<_>>()?;

        Self::from_source(base.try_into()?, variants, info)
    }

    /// Saves `self` to `path` as Xcursor.
    ///
    /// ## Errors
    ///
    /// If filesystem operations fail, or if propagated from [`Xcursor`].
    pub fn save_as_xcursor<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        let file = File::create(path)?;

        // this line is pretty important. reduces syscalls by like 500x
        let mut writer = BufWriter::new(file);

        let xcursor = Xcursor::new(self)?;

        xcursor.write(&mut writer)?;
        writer.flush().context("failed to flush Xcursor output")?;

        Ok(())
    }

    /// Tries to use 32x32 as base and checks `icons`. If there are
    /// no 32x32 entries, defaults to dimensions of first entry.
    fn get_base_dimensions(icons: &[&IconDir]) -> (u32, u32) {
        if icons
            .iter()
            .flat_map(|ico| ico.entries())
            .any(|e| (e.width(), e.height()) == (32, 32))
        {
            (32, 32)
        } else {
            let first_entry = &icons[0].entries()[0];
            (first_entry.width(), first_entry.height())
        }
    }

    /// Iterates over the base frames followed by the frames of each other size.
    pub fn frames(&self) -> impl Iterator<Item = &CursorImage> {
        self.images.iter().flat_map(|(_, frames)| frames.inner())
    }

    #[must_use]
    pub fn info(&self) -> Option<String> {
        self.info.clone()
    }

    /// Returns the total number of frames across all sizes.
    #[must_use]
    pub fn num_images(&self) -> usize {
        self.images.iter().map(|(_, frames)| frames.len()).sum()
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::cursors::cursor_image::tests::{BLACK, WHITE};

    /// Generates an animated cursor with ten alternating black and white frames.
    #[expect(clippy::missing_panics_doc)]
    pub fn black_and_white() -> GenericCursor {
        let frames = [
            BLACK.clone(),
            WHITE.clone(),
            BLACK.clone(),
            WHITE.clone(),
            BLACK.clone(),
            WHITE.clone(),
            BLACK.clone(),
            WHITE.clone(),
            BLACK.clone(),
            WHITE.clone(),
        ];

        let frames = CursorImages::try_from(frames.to_vec()).unwrap();
        GenericCursor::from_source(frames, Vec::new(), None).unwrap()
    }
}
