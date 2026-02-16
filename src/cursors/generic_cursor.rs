//! Contains the [`GenericCursor`] struct.
//!
//! This represents a full static/animated cursor.

use crate::{
    cursors::cursor_image::{CursorImage, CursorImages},
    formats::{ani::AniFile, xcursor::Xcursor},
};

use std::{
    fs::{self, File},
    io::{BufWriter, Cursor},
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
    /// The base images, used for scaling.
    base: CursorImages,

    /// Scaled cursors derived from `base`.
    ///
    /// Each vector has the same length as `base`.
    scaled: Vec<CursorImages>,

    /// Used scale factors. Always includes 1.0.
    scale_factors: Vec<f64>,
}

impl GenericCursor {
    /// Trivial constructor. `scale_factors` is inferred from `scaled_images`.
    ///
    /// ## Errors
    ///
    /// - If `base_images` or `scaled_images` is empty.
    /// - If propagated from [`CursorImages`] construction.
    pub(super) fn new(base_images: CursorImages, scaled_images: Vec<CursorImages>) -> Result<Self> {
        if scaled_images.is_empty() {
            bail!("scaled_images can't be empty, call Self::new_unscaled() if this is expected");
        }

        let mut scale_factors = Vec::with_capacity(scaled_images.len());
        scale_factors.push(1.0);

        // used for calculating sf
        let base_nominal = f64::from(base_images.first().nominal_size());
        let base_len = base_images.len();

        for images in &scaled_images {
            if images.len() != base_len {
                bail!(
                    "expected base_len={base_len} images, instead got images.len()={}",
                    images.len()
                );
            }

            let scaled_nominal = f64::from(images.first().nominal_size());
            let scale_factor = scaled_nominal / base_nominal;

            if scale_factors.contains(&scale_factor) {
                bail!(
                    "scaled `GenericCursor` constructor must \
                    have unique scale factors for scaled frames"
                );
            }

            scale_factors.push(scale_factor);
        }

        Ok(Self {
            base: base_images,
            scaled: scaled_images,
            scale_factors,
        })
    }

    /// Constructor without `scaled`.
    #[must_use]
    pub fn new_unscaled(base_images: CursorImages) -> Self {
        Self {
            base: base_images,
            scaled: Vec::new(),
            scale_factors: vec![1.0],
        }
    }

    /// Adds scaled [`CursorImage`] from `base` to `scaled`.
    ///
    /// NOTE: Downscaling isn't recommended for pixel-art images.
    ///
    /// ## Errors
    ///
    /// If the newly made [`CursorImage`] doesn't
    /// have a unique (canon) scale factor.
    pub fn add_scale(&mut self, scale_factor: f64, algorithm: ResizeAlg) -> Result<()> {
        // some cursors already store scaled versions
        if self.scale_factors.contains(&scale_factor) {
            eprintln!("scale_factor={scale_factor} already added, skipping");
            return Ok(());
        }

        self.scale_factors.push(scale_factor);

        let scaled_images: Vec<CursorImage> = self
            .base
            .inner()
            .iter()
            .map(|c| c.scaled_to(scale_factor, algorithm))
            .collect::<Result<_>>()?;

        self.scaled.push(scaled_images.try_into()?);

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
            bail!("path has no extension; expected 'cur' or 'ani'");
        };

        let ext = ext.to_ascii_lowercase();

        let cursor = if ext == "cur" {
            Self::from_cur_path(path)
        } else if ext == "ani" {
            Self::from_ani_path(path)
        } else {
            bail!(
                "expected extension 'cur' or 'ani' for path, got ext={}",
                ext.display()
            );
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
    pub fn from_cur_path<P: AsRef<Path>>(cur_path: P) -> Result<Self> {
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
            let image = CursorImage::from_entry(entry, 0)?;

            if image.dimensions() == base_dims {
                base.push(image);
            } else {
                scaled.push(image);
            }
        }

        let base = CursorImages::try_from(base)?;

        if scaled.is_empty() {
            Ok(Self::new_unscaled(base))
        } else {
            Self::new(base, vec![scaled.try_into()?])
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
    pub fn from_ani_path<P: AsRef<Path>>(ani_path: P) -> Result<Self> {
        let ani_blob = fs::read(&ani_path)?;
        let ani_file = AniFile::from_blob(&ani_blob)?;
        let header = &ani_file.header;

        // read each ico frame
        let icos: Vec<IconDir> = ani_file
            .ico_frames
            .into_iter()
            .map(|chunk| IconDir::read(&mut Cursor::new(&chunk.data)))
            .collect::<Result<_, _>>()?;

        // get display order as indices into icos
        let sequence: Option<Vec<usize>> = ani_file
            .sequence
            .map(|chunk| chunk.data.into_iter().map(usize::try_from).collect())
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
            .map_or_else(|| vec![header.jiffy_rate; num_steps], |chunk| chunk.data);

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
                let image = CursorImage::from_entry(entry, delay)?;

                if image.dimensions() == base_dims {
                    base.push(image);
                } else {
                    scaled_ungrouped.push(image);
                }
            }
        }

        let base = CursorImages::try_from(base)?;

        if scaled_ungrouped.is_empty() {
            return Ok(Self::new_unscaled(base));
        }

        // could use hashmap here but ehh
        scaled_ungrouped.sort_unstable_by_key(CursorImage::dimensions);
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

        Self::new(base, scaled)
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

        // this can create partial writes. consider fixing
        xcursor.write(&mut writer)?;

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

    /// Trivial accessor for `base` field.
    #[must_use]
    pub fn base_images(&self) -> &CursorImages {
        &self.base
    }

    /// Trivial accessor for `scaled` field.
    pub fn scaled_images(&self) -> impl Iterator<Item = &CursorImages> {
        self.scaled.iter()
    }

    /// Returns the number of `base` and `scaled` images.
    ///
    /// Prefer this over calling [`Iterator::count`]
    /// on [`Self::joined_images`] or equivalent.
    #[must_use]
    pub const fn num_images(&self) -> usize {
        (self.scaled.len() + 1) * self.base.len()
    }

    /// Returns an iterator joining `base` and `scaled` flattened
    /// over [`CursorImage`] (rather than [`CursorImages`]).
    pub fn joined_images(&self) -> impl Iterator<Item = &CursorImage> {
        self.base
            .inner()
            .iter()
            .chain(self.scaled.iter().flat_map(CursorImages::inner))
    }
}
