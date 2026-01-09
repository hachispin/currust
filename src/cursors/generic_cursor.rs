//! Contains the [`GenericCursor`] struct.

use super::{
    ani::AniFile,
    cursor_image::{CursorImage, ScalingType},
    xcursor::{bundle_images, construct_images, save_images},
};

use std::{
    fs::{self, File},
    io::Cursor,
    mem,
    path::Path,
};

use anyhow::{Context, Result, anyhow, bail};
use ico::IconDir;

/// Represents a generic cursor.
///
/// `images` is guaranteed to not have any images
/// that share the same dimensions.
#[derive(Debug)]
pub struct GenericCursor {
    /// The base images, used for scaling.
    ///
    /// - For static cursors, this should have one
    ///   [`CursorImage`] with a `delay` of zero.
    /// - For animated cursors, this should have multiple
    ///   [`CursorImage`]s with non-zero `delay` fields.
    ///
    /// All images here must have the **same dimensions**.
    base: Vec<CursorImage>,

    /// Scaled cursors derived from `base`.
    ///
    /// Each inner vector should have the same length as `base`.
    scaled: Vec<Vec<CursorImage>>,

    /// Used scale factors. Always includes 1.0.
    ///
    /// Downscaled factors are added as 1/SF.
    scale_factors: Vec<f64>,
}

impl GenericCursor {
    /// Trivial constructor.
    ///
    /// ## Errors
    ///
    /// If any image in `base_images` have different dimensions.
    pub fn new(base_images: Vec<CursorImage>) -> Result<Self> {
        if base_images.is_empty() {
            bail!("`base_images` can't be empty");
        }

        if !Self::has_consistent_dimensions(&base_images) {
            bail!(
                "`GenericCursor` can't be constructed with \
                base images that don't have the same dimensions"
            );
        }

        Ok(Self {
            base: base_images,
            scaled: Vec::new(),
            scale_factors: vec![1.0],
        })
    }

    /// Constructor for cursors that already store multiple sizes.
    /// This also infers scale factors from `scaled_images`.
    ///
    /// If `scaled_images` is empty, this is the same as [`Self::new`].
    ///
    /// ## Errors
    ///
    /// - If `base_images` is empty.
    /// - If `base_images` or each `scaled_image` has inconsistent
    ///   dimensions for each frame (for animated cursors).
    /// - If any [`Vec<CursorImage>`] differs in length (may be missing frames?)
    pub fn new_with_scaled(
        base_images: Vec<CursorImage>,
        scaled_images: Vec<Vec<CursorImage>>,
    ) -> Result<Self> {
        if scaled_images.is_empty() {
            return Self::new(base_images);
        }

        let mut scale_factors = Vec::with_capacity(scaled_images.len());
        let base_len = base_images.len();
        let base_dims = base_images[0].dimensions();

        // used for calculating sf
        let base_nominal = f64::from(base_dims.0.max(base_dims.1));

        if !Self::has_consistent_dimensions(&base_images) {
            bail!(
                "`GenericCursor` can't be constructed with \
                base images that don't have the same dimensions"
            );
        }

        for images in &scaled_images {
            if images.is_empty() {
                bail!("`scaled_images` can't contain empty vectors");
            }

            if images.len() != base_len {
                bail!(
                    "expected base_len={base_len} images, instead got images.len()={}",
                    images.len()
                );
            }

            if !Self::has_consistent_dimensions(images) {
                bail!(
                    "scaled `GenericCursor` constructor must \
                    have consistent dimensions for scaled frames"
                );
            }

            let scaled_dims = images[0].dimensions();
            let scaled_nominal = f64::from(scaled_dims.0.max(scaled_dims.1));
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

    /// Helper function for checking for inconsistent
    /// dimensions within a [`Vec<CursorImage>`].
    ///
    /// NOTE: If `images` is empty, this returns `true`.
    #[inline]
    fn has_consistent_dimensions(images: &[CursorImage]) -> bool {
        if images.is_empty() {
            return true;
        }

        let expected_dims = images[0].dimensions();
        images.iter().all(|img| img.dimensions() == expected_dims)
    }

    /// Adds scaled [`CursorImage`] from `base` to `scaled`.
    ///
    /// NOTE: Downscaling isn't recommended for pixel-art images.
    ///
    /// ## Errors
    ///
    /// If the newly made [`CursorImage`] doesn't
    /// have a unique (canon) scale factor.
    pub fn add_scale(&mut self, scale_factor: u32, scale_type: ScalingType) -> Result<()> {
        let canon_scale_factor: f64 = match scale_type {
            ScalingType::Upscale => f64::from(scale_factor),
            ScalingType::Downscale => 1.0 / f64::from(scale_factor),
        };

        if self.scale_factors.contains(&canon_scale_factor) {
            bail!("scale_factor={scale_factor} already added");
        }

        self.scale_factors.push(canon_scale_factor);

        let scaled_images: Vec<CursorImage> = self
            .base
            .iter()
            .map(|c| c.scaled_to(scale_factor, scale_type))
            .collect::<Result<_>>()?;

        self.scaled.push(scaled_images);

        Ok(())
    }

    /// Reads and parses a cursor from `cur_path`, which
    /// must be a path to a Windows cursor file (i.e, CUR).
    ///
    /// ## Errors
    ///
    /// If a file handle to `cur_path` can't be opened,
    /// or the file stored is not a CUR file.
    ///
    /// This also checks for the `.cur` extension.
    pub fn from_cur_path<P: AsRef<Path>>(cur_path: P) -> Result<Self> {
        let cur_path = cur_path.as_ref();
        let cur_path_display = cur_path.display();

        if cur_path.extension().is_none_or(|ext| ext != "cur") {
            bail!("expected {cur_path_display} to have extension 'cur'")
        }

        let handle = File::open(cur_path)
            .with_context(|| format!("failed to read from cur_path={cur_path_display}"))?;

        let icon_dir = IconDir::read(handle).with_context(|| {
            format!("failed to read `IconDir` from cur_path={cur_path_display}")
        })?;

        let entries = icon_dir.entries();

        if entries.is_empty() {
            bail!("no stored images found in {cur_path_display}");
        }

        if entries.len() != 1 {
            eprintln!("[warning] parsing CUR file with more than one stored image");
        }

        let mut images = Vec::with_capacity(entries.len());

        for entry in entries {
            let image = entry.decode()?;
            let hotspot = image.cursor_hotspot().ok_or_else(|| {
                anyhow!("provided cur_path={cur_path_display} must be to CUR, not ICO")
            })?;

            let image = CursorImage::new(
                image.width(),
                image.height(),
                u32::from(hotspot.0),
                u32::from(hotspot.1),
                image.into_rgba_data(),
            )?;

            images.push(image);
        }

        Self::new(images)
    }

    /// Parses `ani_path`.
    ///
    /// ## Errors
    ///
    /// Path `ani_path` is not to a valid ANI cursor.
    ///
    /// This also checks for the `.ani` extension.
    pub fn from_ani_path<P: AsRef<Path>>(ani_path: P) -> Result<Self> {
        let ani_path = ani_path.as_ref();
        let ani_path_display = ani_path.display();

        if ani_path.extension().is_none_or(|ext| ext != "ani") {
            bail!("expected {ani_path_display} to have extension 'ani'")
        }

        let ani_blob = fs::read(ani_path)?;
        let ani_file = AniFile::from_blob(&ani_blob)?;
        let header = ani_file.header;

        let icos: Vec<IconDir> = ani_file
            .ico_frames
            .into_iter()
            .map(|chunk| IconDir::read(&mut Cursor::new(&chunk.data)))
            .collect::<Result<_, _>>()?;

        let sequence: Option<Vec<usize>> = ani_file
            .sequence
            .map(|chunk| chunk.data.into_iter().map(usize::try_from).collect())
            .transpose()?;

        let sequenced_icos: Vec<&IconDir> = sequence.map_or(icos.iter().collect(), |v| {
            v.into_iter().map(|idx| &icos[idx]).collect()
        });

        let num_steps = usize::try_from(header.num_steps)?;
        let delays_jiffies = ani_file
            .rate
            .map_or(vec![header.jiffy_rate; num_steps], |chunk| chunk.data);

        // jiffies are 1/60th of a second
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let delays_ms: Vec<u32> = delays_jiffies
            .into_iter()
            .map(|j| (f64::from(j) * 1000.0 / 60.0).round() as u32)
            .collect();

        let first_entry = &sequenced_icos[0].entries()[0];
        let base_dims = (first_entry.width(), first_entry.height());
        let mut base: Vec<CursorImage> = Vec::new();
        let mut scaled_ungrouped: Vec<CursorImage> = Vec::new();

        for (ico, delay) in sequenced_icos.iter().zip(delays_ms) {
            let entries = ico.entries();

            for entry in entries {
                let rgba = entry.decode()?.into_rgba_data();
                let (hotspot_x, hotspot_y) = entry.cursor_hotspot().ok_or(anyhow!(
                    "expected stored ANI frames to be CUR, instead got ICO \
                    are you sure {ani_path_display} is meant for cursors?"
                ))?;

                let image = CursorImage::new_with_delay(
                    entry.width(),
                    entry.height(),
                    hotspot_x.into(),
                    hotspot_y.into(),
                    rgba,
                    delay,
                )?;

                if image.dimensions() == base_dims {
                    base.push(image);
                } else {
                    scaled_ungrouped.push(image);
                }
            }
        }

        if scaled_ungrouped.is_empty() {
            return Self::new(base);
        }

        scaled_ungrouped.sort_unstable_by_key(CursorImage::dimensions);

        let scaled_ungrouped = scaled_ungrouped;
        let mut scaled = Vec::new();
        let mut current_dims = scaled_ungrouped[0].dimensions();
        let mut buffer = Vec::new();

        // group by dimensions
        for image in scaled_ungrouped {
            if image.dimensions() != current_dims {
                scaled.push(mem::take(&mut buffer));
                current_dims = image.dimensions();
            }

            buffer.push(image);
        }

        // push anything left
        if !buffer.is_empty() {
            scaled.push(buffer);
        }

        GenericCursor::new_with_scaled(base, scaled)
    }

    /// Saves `cursor` to `path` in Xcursor format.
    ///
    /// ## Errors
    ///
    /// If `path` has no `&str` representation, or errors
    /// from the `unsafe` helper functions are propagated.
    pub fn save_as_xcursor<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        let joined: Vec<&CursorImage> = self.joined_images().collect();

        let path_str = path
            .to_str()
            .ok_or_else(|| anyhow!("failed to convert path={} to &str", path.display()))?;

        let mut images_vec: Vec<_> = joined
            .into_iter()
            .map(construct_images)
            .collect::<Result<_>>()?;

        let images = unsafe { bundle_images(&mut images_vec) }?;

        unsafe {
            save_images(path_str, &images)?;
        }

        Ok(())
    }

    /// Trivial accessor for `base` field.
    #[must_use]
    pub fn base_images(&self) -> &[CursorImage] {
        &self.base
    }

    /// Trivial accessor for `scaled` field.
    ///
    /// This returns an iterator over `&[CursorImage]`.
    pub fn scaled_images(&self) -> impl Iterator<Item = &[CursorImage]> {
        self.scaled.iter().map(Vec::as_slice)
    }

    /// Trivial accessor for `scaled` field, flattened.
    ///
    /// This returns an iterator over `&CursorImage`.
    pub fn scaled_images_flat(&self) -> impl Iterator<Item = &CursorImage> {
        self.scaled.iter().flat_map(Vec::as_slice)
    }

    /// Returns an iterator joining `base` and `scaled`, flattened.
    pub fn joined_images(&self) -> impl Iterator<Item = &CursorImage> {
        self.base.iter().chain(self.scaled_images_flat())
    }
}
