//! Contains the [`GenericCursor`] struct, which is used
//! as a medium between Windows and Linux cursors.

use super::xcursor::{bundle_images, construct_images, save_images};
use crate::scaling::{scale_box_average, scale_nearest};

use std::{fs::File, path::Path};

use anyhow::{Context, Result, bail};
use ico::IconDir;

/// Represents a generic cursor *image*.
///
/// An actual cursor is usually expressed as a
/// vector of cursor images. See [`GenericCursor`].
#[derive(Debug, Clone)]
pub struct CursorImage {
    width: u32,
    height: u32,
    hotspot_x: u32,
    hotspot_y: u32,
    rgba: Vec<u8>,
    delay: u32,
}

impl CursorImage {
    /// A delay value of zero is used for static (i.e, non-animated) cursors.
    pub(crate) const STATIC_DELAY: u32 = 0;
    /// The max upscaling factor for images.
    pub const MAX_UPSCALE_FACTOR: u32 = 20;
    /// The max downscaling factor for images.
    pub const MAX_DOWNSCALE_FACTOR: u32 = 5;

    /// Contructor for a static [`CursorImage`].
    /// The `delay` field is set to zero.
    ///
    /// ## Errors
    ///
    /// - If [`TryInto`] conversions fail.
    /// - If `hotspot_x > width` and ditto for y.
    /// - If `width * height * 4 != rgba.len()`.
    pub fn new(
        width: u32,
        height: u32,
        hotspot_x: u32,
        hotspot_y: u32,
        rgba: Vec<u8>,
    ) -> Result<Self> {
        if width == 0 {
            bail!("width cannot be zero");
        }

        if height == 0 {
            bail!("height cannot be zero")
        }

        if hotspot_x > width {
            bail!("hotspot_x={hotspot_x} cannot be greater than width={width}");
        }

        if hotspot_y > height {
            bail!("hotspot_y={hotspot_y} cannot be greater than height={height}");
        }

        if (width * height * 4) != rgba.len().try_into()? {
            bail!(
                "Expected rgba.len()={}, instead got rgba.len()={}",
                width * height * 4,
                rgba.len()
            );
        }

        if width != height {
            eprintln!(
                "Warning: width={width} and height={height} \
                aren't equal, this may cause odd behaviour"
            );
        }

        Ok(Self {
            width,
            height,
            hotspot_x,
            hotspot_y,
            rgba,
            delay: Self::STATIC_DELAY,
        })
    }

    /// Constructor with `delay` field.
    ///
    /// ## Errors
    ///
    /// See [`Self::new`].
    pub fn new_with_delay(
        width: u32,
        height: u32,
        hotspot_x: u32,
        hotspot_y: u32,
        rgba: Vec<u8>,
        delay: u32,
    ) -> Result<Self> {
        let mut cursor = Self::new(width, height, hotspot_x, hotspot_y, rgba)?;
        cursor.delay = delay;

        Ok(cursor)
    }

    /// Returns a new [`CursorImage`] scaled *up* to `scale_factor` using
    /// [nearest-neighbour](https://en.wikipedia.org/wiki/Image_scaling#Nearest-neighbor_interpolation)
    /// scaling.
    ///
    /// ## Errors
    ///
    /// If `scale_factor` is greater than [`Self::MAX_UPSCALE_FACTOR`].
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn upscaled_to(&self, scale_factor: u32) -> Result<Self> {
        if scale_factor > Self::MAX_UPSCALE_FACTOR {
            bail!(
                "scale_factor={scale_factor} can't be greater than MAX_SCALE_FACTOR={}",
                Self::MAX_UPSCALE_FACTOR
            );
        }

        let (width, height) = self.dimensions();
        let (scaled_width, scaled_height) = (width * scale_factor, height * scale_factor);
        let scaled_rgba = scale_nearest(self.rgba(), width, height, scaled_width, scaled_height);

        let (hotspot_x, hotspot_y) = self.hotspot();
        let (scaled_hotspot_x, scaled_hotspot_y) =
            (hotspot_x * scale_factor, hotspot_y * scale_factor);

        Ok(Self {
            width: scaled_width,
            height: scaled_height,
            hotspot_x: scaled_hotspot_x,
            hotspot_y: scaled_hotspot_y,
            rgba: scaled_rgba,
            delay: self.delay,
        })
    }

    /// Returns a new [`CursorImage`] scaled *down* to `scale_factor` using
    /// [box averaging](https://en.wikipedia.org/wiki/Image_scaling#Box_sampling).
    ///
    /// The actual "scale factor" would be `1/scale_factor` here.
    ///
    /// ## Errors
    ///
    /// If `scale_factor` is greater than [`Self::MAX_DOWNSCALE_FACTOR`]
    pub fn downscaled_to(&self, scale_factor: u32) -> Result<Self> {
        if scale_factor > Self::MAX_DOWNSCALE_FACTOR {
            bail!(
                "scale_factor={scale_factor} can't be greater than MAX_DOWNSCALE_FACTOR={}",
                Self::MAX_DOWNSCALE_FACTOR
            )
        }

        let (width, height) = self.dimensions();
        let (scaled_width, scaled_height) = (width / scale_factor, height / scale_factor);
        let scaled_rgba =
            scale_box_average(self.rgba(), width, height, scaled_width, scaled_height);

        let (hotspot_x, hotspot_y) = self.hotspot();
        let (scaled_hotspot_x, scaled_hotspot_y) =
            (hotspot_x / scale_factor, hotspot_y / scale_factor);

        Ok(Self {
            width: scaled_width,
            height: scaled_height,
            hotspot_x: scaled_hotspot_x,
            hotspot_y: scaled_hotspot_y,
            rgba: scaled_rgba,
            delay: self.delay,
        })
    }

    /// Returns image dimensions as (width, height).
    #[must_use]
    pub const fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Returns hotspot coordinates as (x, y).
    #[must_use]
    pub const fn hotspot(&self) -> (u32, u32) {
        (self.hotspot_x, self.hotspot_y)
    }

    /// Returns a reference to the stored RGBA.
    #[must_use]
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}

/// Represents a generic cursor.
///
/// `images` is guaranteed to not have any images
/// that share the same nominal sizes.
#[derive(Debug)]
pub struct GenericCursor {
    /// Scaled images derived from `base`.
    scaled: Vec<CursorImage>,
    /// The base images, used for scaling.
    ///
    /// For static cursors, this contains one
    /// [`CursorImage`] with a `delay` field of 0.
    ///
    /// For animated cursors, this contains multiple
    /// [`CursorImage`]s with non-zero `delay` fields.
    ///
    /// All images here must have the **same nominal sizes**.
    base: Vec<CursorImage>,
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

        let expected_dimensions = base_images[0].dimensions();

        for image in &base_images {
            let dims = image.dimensions();

            if dims != expected_dimensions {
                bail!(
                    "`GenericCursor` can't be constructed with \
                    base images that don't have the same dimensions"
                );
            }
        }

        Ok(Self {
            base: base_images,
            scaled: Vec::new(),
        })
    }

    /// Helper function for [`Self::add_scale`].
    fn has_nominal_size(&self, nominal_size: u32) -> bool {
        for image in &self.scaled {
            let dims = image.dimensions();

            if dims.0.max(dims.1) == nominal_size {
                return true;
            }
        }

        false
    }

    /* TODO: Deduplicate upscaling and downscaling code */

    /// Adds an *upscaled* [`CursorImage`] to [`Self::images`]. This
    /// scales based off of the first element in [`Self::images`].
    ///
    /// ## Errors
    ///
    /// If the newly made [`CursorImage`] doesn't
    /// have a unique nominal size.
    pub fn add_upscale(&mut self, scale_factor: u32) -> Result<()> {
        let base_images = &self.base;

        for base_image in base_images {
            let dims = base_image.dimensions();
            let scaled_dims = (dims.0 * scale_factor, dims.1 * scale_factor);
            let scaled_nominal = scaled_dims.0.max(scaled_dims.1);

            if self.has_nominal_size(scaled_nominal) {
                bail!("duplicate nominal size");
            }

            let scaled_image = base_image.upscaled_to(scale_factor)?;
            self.scaled.push(scaled_image);
        }

        Ok(())
    }

    /// Adds a *downscaled* [`CursorImage`] to [`Self::images`]. This
    /// scales based off of the first element in [`Self::images`].
    ///
    /// ## Errors
    ///
    /// If the newly made [`CursorImage`] doesn't
    /// have a unique nominal size.
    pub fn add_downscale(&mut self, scale_factor: u32) -> Result<()> {
        let base_images = &self.base;

        for base_image in base_images {
            let dims = base_image.dimensions();
            let scaled_dims = (dims.0 * scale_factor, dims.1 * scale_factor);
            let scaled_nominal = scaled_dims.0.max(scaled_dims.1);

            if self.has_nominal_size(scaled_nominal) {
                bail!("duplicate nominal size");
            }

            let scaled_image = base_image.downscaled_to(scale_factor)?;
            self.scaled.push(scaled_image);
        }

        Ok(())
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
        let cur_path_display = cur_path.display();

        let handle = File::open(cur_path)
            .with_context(|| format!("failed to read from cur_path={cur_path_display}"))?;

        let icon_dir = IconDir::read(handle).with_context(|| {
            format!("failed to read `IconDir` from cur_path={cur_path_display}")
        })?;

        let entries = icon_dir.entries();

        if entries.is_empty() {
            bail!("No stored images found in {cur_path_display}");
        }

        if entries.len() != 1 {
            eprintln!("Warning: parsing CUR file with more than one stored image");
        }

        let mut images = Vec::with_capacity(entries.len());

        // this is written as a for loop but only one image should be expected
        // windows already scales cursors so storing more is redundant
        // ughhh... this opens up so many edge cases
        for entry in entries {
            let image = entry.decode()?;
            let hotspot = image.cursor_hotspot().ok_or_else(|| {
                anyhow::anyhow!("provided cur_path={cur_path_display} must be to CUR, not ICO")
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

    /// Saves `cursor` to `path` in Xcursor format.
    ///
    /// ## Errors
    ///
    /// If `path` has no `&str` representation, or errors
    /// from the `unsafe` helper functions are propagated.
    pub fn save_as_xcursor<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        let joined = self.joined_images();
        let cursor = joined.as_slice();

        let path_str = path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("failed to convert path={} to &str", path.display()))?;

        let mut images_vec = Vec::with_capacity(cursor.len());

        for c in cursor {
            // drop called on XcursorImage if propagated
            let image = construct_images(c)?;
            images_vec.push(image);
        }

        let images = unsafe { bundle_images(&mut images_vec) }?;

        unsafe {
            // drop called on each stored XcursorImage if propagated
            save_images(path_str, &images)?;
        }

        Ok(())
    }

    /// Returns a vector joining `base` and `scaled`.
    #[must_use]
    pub fn joined_images(&self) -> Vec<CursorImage> {
        let mut images = self.base.clone();
        images.extend(self.scaled.clone());

        images
    }

    /// Trivial accessor for `base` field.
    #[must_use]
    pub fn base_images(&self) -> &[CursorImage] {
        &self.base
    }

    /// Trivial accessor for `scaled` field.
    #[must_use]
    pub fn scaled_images(&self) -> &[CursorImage] {
        &self.scaled
    }
}
