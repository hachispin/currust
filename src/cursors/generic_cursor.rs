//! Contains the [`GenericCursor`] struct.
//!
//! This represents a full static/animated cursor.

use crate::formats::{ani::AniFile, xcursor::Xcursor};

use super::{
    cursor_image::{CursorImage, CursorImages},
    cursor_role::CursorRole,
};

use std::{
    fs::{self, File},
    io::{BufWriter, Cursor, Write},
    mem,
    path::Path,
};

use anyhow::{Context, Result, bail};
use binrw::BinWrite;
use fast_image_resize::ResizeAlg;
use ico::IconDir;

/// Represents a generic cursor.
#[derive(Debug)]
pub struct GenericCursor {
    /// `(scale_factor, cursorimages)` pairs. Base will have a scale factor of 1.0.
    images: Vec<(f64, CursorImages)>,
    /// The cursors semantic type.
    role: Option<CursorRole>,
    /// Info stored within the cursor.
    ///
    /// Both ANI and Xcursor have the capability of storing metadata similar
    /// to this. ANI uses it's "INFO" chunk, while Xcursor uses comment chunks.
    info: Option<String>,
}

impl GenericCursor {
    /// Constructor with some validation-ish.
    ///
    /// ## Errors
    ///
    /// If `images` is empty.
    fn new(
        images: Vec<(f64, CursorImages)>,
        role: Option<CursorRole>,
        info: Option<String>,
    ) -> Result<Self> {
        if images.is_empty() {
            bail!("can't create a cursor with no images");
        }

        if !images.iter().any(|img| img.0 == 1.0) {
            bail!("no base variant (sf = 1.0) found");
        }

        Ok(Self { images, role, info })
    }

    /// Adds scaled [`CursorImage`] from `base` to `scaled`.
    ///
    /// NOTE: Downscaling isn't recommended for pixel-art images.
    ///
    /// ## Errors
    ///
    /// If the newly made [`CursorImage`] doesn't have a unique (canon) scale factor.
    pub fn add_scale(&mut self, scale_factor: f64, algorithm: ResizeAlg) -> Result<()> {
        // some cursors already store scaled versions
        if self.scale_factors().into_iter().all(|sf| sf != 1.0) {
            eprintln!("scale_factor={scale_factor} already added, skipping");
            return Ok(());
        }

        let scaled_images: Vec<CursorImage> = self
            .base_images()
            .inner()
            .into_iter()
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

        let base = CursorImages::try_from(base)?;

        if scaled.is_empty() {
            Ok(Self::new_unscaled(base, None))
        } else {
            Self::new(base, scaled, None)
        }
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
        let mut base = Vec::new();
        let mut scaled_ungrouped = Vec::new();

        for (ico, delay) in sequenced_icos.iter().zip(delays_ms) {
            let entries = ico.entries();

            for entry in entries {
                let image = CursorImage::from_entry(entry, base_nominal_size, delay)?;

                if image.dimensions() == base_dims {
                    base.push(image);
                } else {
                    scaled_ungrouped.push(image);
                }
            }
        }

        let base = CursorImages::try_from(base)?;

        if scaled_ungrouped.is_empty() {
            return Ok(Self::new_unscaled(base, info));
        }

        scaled_ungrouped.sort_by_key(CursorImage::dimensions);
        let scaled_ungrouped = scaled_ungrouped;
        let mut scaled = Vec::new();
        let mut buffer = Vec::new();
        let mut current_dims = scaled_ungrouped[0].dimensions();

        // group by dimensions
        for image in scaled_ungrouped {
            if image.dimensions() != current_dims {
                scaled.push((mem::take(&mut buffer)).try_into()?);
                current_dims = image.dimensions();
            }

            buffer.push(image);
        }

        // push anything left
        if !buffer.is_empty() {
            scaled.push(buffer.try_into()?);
        }

        Self::new(base, scaled, info)
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

    /// Helper function for [`Self::from_ani_path`].
    ///
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

    /// Returns the base [`CursorImages`] (sf = 1.0).
    #[must_use]
    pub fn base_images(&self) -> &CursorImages {
        &self
            .images
            .iter()
            .find(|img| img.0 == 1.0)
            .expect("no base cursor frame found?")
            .1
    }

    /// Returns all non-base [`CursorImages`] (sf != 1.0).
    pub fn scaled_images(&self) -> impl Iterator<Item = &CursorImages> {
        self.images
            .iter()
            .filter(|img| img.0 != 1.0)
            .map(|img| &img.1)
    }

    pub fn scale_factors(&self) -> impl Iterator<Item = f64> {
        self.images.iter().map(|img| img.0)
    }

    #[must_use]
    pub fn info(&self) -> Option<String> {
        self.info.clone()
    }

    /// Returns the number of `base` and `scaled` images/frames.
    ///
    /// Prefer this over calling [`Iterator::count`] on [`Self::joined_images`] or equivalent.
    #[must_use]
    pub fn num_images(&self) -> usize {
        self.images.iter().fold(0, |acc, x| acc + x.1.len())
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
        GenericCursor::new(vec![(1.0, frames)], None, None).unwrap()
    }
}
