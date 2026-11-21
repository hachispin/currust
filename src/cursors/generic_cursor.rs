//! Contains the [`GenericCursor`] struct.
//!
//! This represents a full static/animated cursor.

use crate::formats::{ani::AniFile, xcursor::Xcursor};

use super::{cursor_image::CursorImage, cursor_role::CursorRole};

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
    /// Source and generated frame sequences paired with their scale factors.
    ///
    /// The first sequence is the canonical base at scale 1.0, chosen by
    /// [`Self::get_base_dimensions`]. For a source with 32x32 and 64x64 frames,
    /// `--scale-to 2.0` targets the 64x64 size that is already present.
    images: Vec<(f64, Vec<CursorImage>)>,
    /// The cursor's role when used in a theme.
    pub role: Option<CursorRole>,
    /// Info stored within the cursor.
    ///
    /// For ANI this is its "INFO" chunk, while for Xcursor this is a comment chunk.
    info: Option<String>,
}

impl GenericCursor {
    fn from_source(
        base: Vec<CursorImage>,
        variants: impl IntoIterator<Item = Vec<CursorImage>>,
        info: Option<String>,
    ) -> Result<Self> {
        let base_frame = base.first().context("no base frames found")?;
        let base_size = base_frame.nominal_size;
        let base_len = base.len();
        let mut images: Vec<(f64, Vec<CursorImage>)> = Vec::new();

        for mut frames in std::iter::once(base).chain(variants) {
            if frames.len() != base_len {
                bail!("expected {base_len} frames per size, got {}", frames.len());
            }

            CursorImage::validate_frames(&mut frames)?;

            let source_size = frames[0].nominal_size;

            if images
                .iter()
                .any(|(_, stored)| stored[0].nominal_size == source_size)
            {
                bail!("multiple source cursor variants have nominal size {source_size}");
            }

            let scale_factor = f64::from(source_size) / f64::from(base_size);
            images.push((scale_factor, frames));
        }

        Ok(Self {
            images,
            role: None,
            info,
        })
    }

    /// Sets the nominal size of the base and updates each variant by its stored scale.
    ///
    /// ## Errors
    ///
    /// If the base size is zero.
    pub fn set_base_nominal_size(&mut self, size: u32) -> Result<()> {
        if size == 0 {
            bail!("nominal size can't be zero");
        }

        let nominal_sizes = self
            .images
            .iter()
            .map(|(scale, _)| {
                let nominal_size = (f64::from(size) * scale).floor().max(1.0);

                #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                Ok(nominal_size as u32)
            })
            .collect::<Result<Vec<_>>>()?;

        for ((_, frames), nominal_size) in self.images.iter_mut().zip(nominal_sizes) {
            for frame in frames {
                frame.nominal_size = nominal_size;
            }
        }

        Ok(())
    }

    /// Adds a variant scaled from the canonical base if its scale and nominal size are new.
    ///
    /// ## Errors
    ///
    /// If resizing fails.
    pub fn add_scale(&mut self, scale_factor: f64, algorithm: ResizeAlg) -> Result<()> {
        let base = &self.images[0].1;
        #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let nominal_size = ((f64::from(base[0].nominal_size) * scale_factor).floor() as u32).max(1);

        #[expect(
            clippy::float_cmp,
            reason = "skip scales already present at the requested factor"
        )]
        if self
            .images
            .iter()
            .any(|(scale, frames)| *scale == scale_factor || frames[0].nominal_size == nominal_size)
        {
            eprintln!("scale_factor={scale_factor} already exists, skipping");
            return Ok(());
        }

        let scaled_frames = base
            .iter()
            .map(|c| c.scaled_to(scale_factor, nominal_size, algorithm))
            .collect::<Result<_>>()?;

        self.images.push((scale_factor, scaled_frames));

        Ok(())
    }

    /// Reads the file and parses based on extension.
    ///
    /// ## Errors
    ///
    /// If `path` has no extension or an extension that isn't "ani" or "cur".
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();

        let Some(ext) = path.extension() else {
            bail!("path has no extension; expected 'cur' or 'ani'")
        };

        let ext = ext.to_ascii_lowercase();

        if ext == "cur" {
            Self::from_cur_path(path)
        } else if ext == "ani" {
            Self::from_ani_path(path)
        } else {
            bail!(
                "expected extension 'cur' or 'ani' for path, got ext={}",
                ext.display()
            )
        }
    }

    /// Reads and parses a cursor from `cur_path`, which
    /// must be a path to a Windows cursor file (i.e, CUR).
    ///
    /// ## Errors
    ///
    /// If a file handle to `cur_path` can't be opened,
    /// or the file stored is not a CUR file.
    pub fn from_cur_path<P: AsRef<Path>>(cur_path: P) -> Result<Self> {
        let cur_path = cur_path.as_ref();
        let handle = fs::read(cur_path).context("(filesystem) failed to read")?;
        let icon_dir = IconDir::read(Cursor::new(handle)).context("failed to read `IconDir`")?;
        let entries = icon_dir.entries();

        if entries.is_empty() {
            bail!("no stored images found");
        }

        let mut base = Vec::new();
        let mut variants = Vec::new();
        let base_dims = Self::get_base_dimensions(&[&icon_dir]);

        for entry in entries {
            let image = CursorImage::from_entry(entry, 0)?;

            if image.dimensions() == base_dims {
                base.push(image);
            } else {
                variants.push(vec![image]);
            }
        }

        Self::from_source(base, variants, None)
    }

    /// Parses `ani_path`.
    ///
    /// ## Errors
    ///
    /// - `ani_path` fails to be parsed as an [`IconDir`]
    /// - Stored RGBA in ICO frames fail to be decoded.
    /// - Frame sequences have incompatible dimensions, delays, or lengths.
    /// - [`TryInto`] conversions fail (between primitive types).
    pub fn from_ani_path<P: AsRef<Path>>(ani_path: P) -> Result<Self> {
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
                let image = CursorImage::from_entry(entry, delay)?;
                by_dimensions
                    .entry(image.dimensions())
                    .or_default()
                    .push(image);
            }
        }

        let base = by_dimensions
            .remove(&base_dims)
            .context("no base frames found")?;

        Self::from_source(base, by_dimensions.into_values(), info)
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
        self.images.iter().flat_map(|(_, frames)| frames)
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

        GenericCursor::from_source(frames.to_vec(), [], None).unwrap()
    }
}
