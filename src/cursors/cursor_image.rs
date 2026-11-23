//! Frames of static and animated cursors.

use std::fmt;

use anyhow::{Context, Result, bail};
use fast_image_resize::{
    PixelType, ResizeAlg, ResizeOptions, Resizer,
    images::{Image, ImageRef},
};
use ico::{IconDirEntry, ResourceType};

/// Represents a generic cursor image.
pub struct CursorImage {
    width: u32,
    height: u32,
    /// The size used by Xcursor to select a frame sequence.
    pub nominal_size: u32,
    hotspot_x: u32,
    hotspot_y: u32,
    rgba: Vec<u8>,
    delay: u32,
}

impl CursorImage {
    /// Constructor for a [`CursorImage`].
    ///
    /// ## Errors
    ///
    /// - If [`TryInto`] conversions fail.
    /// - If `hotspot_x > width` and ditto for y.
    /// - If `width * height * 4 != rgba.len()`.
    fn new(
        width: u32,
        height: u32,
        nominal_size: u32,
        hotspot_x: u32,
        hotspot_y: u32,
        rgba: Vec<u8>,
        delay: u32,
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

        Ok(Self {
            width,
            height,
            nominal_size,
            hotspot_x,
            hotspot_y,
            rgba,
            delay,
        })
    }

    /// Helper function for constructing from an `entry`.
    ///
    /// Entries don't store delay unlike Xcursor, so it's a separate parameter.
    ///
    /// ## Errors
    ///
    /// If the entry isn't a cursor (no hotspot), or RGBA fails to decode.
    pub(super) fn from_entry(entry: &IconDirEntry, delay: u32) -> Result<Self> {
        if entry.resource_type() == ResourceType::Icon {
            bail!(
                "can't create CursorImage with resource_type={:?}",
                ResourceType::Icon
            );
        }

        let (hotspot_x, hotspot_y) = entry
            .cursor_hotspot()
            .context("failed to extract hotspot to construct CursorImage")?;

        let rgba = entry
            .decode()
            .context("failed to decode RGBA to construct CursorImage")?
            .into_rgba_data();

        Self::new(
            entry.width(),
            entry.height(),
            entry.width().max(entry.height()),
            hotspot_x.into(),
            hotspot_y.into(),
            rgba,
            delay,
        )
    }

    /// Returns a new [`CursorImage`] scaled to `scale_factor`.
    ///
    /// ## Errors
    ///
    /// If resizing or [`Image`]/[`ImageRef`] constructors fail.
    pub(super) fn scaled_to(
        &self,
        scale_factor: f64,
        nominal_size: u32,
        algorithm: ResizeAlg,
    ) -> Result<Self> {
        let (w1, h1) = self.dimensions();
        let (w2, h2) = Self::scale_point((w1, h1), scale_factor);
        let (hx2, hy2) = Self::scale_point(self.hotspot(), scale_factor);
        let src = ImageRef::new(w1, h1, self.rgba(), PixelType::U8x4)?;
        let mut dst = Image::new(w2, h2, PixelType::U8x4);
        let mut resizer = Resizer::new();
        let options = ResizeOptions::new().resize_alg(algorithm);
        resizer.resize(&src, &mut dst, &options)?;

        Self::new(w2, h2, nominal_size, hx2, hy2, dst.into_vec(), self.delay)
    }

    /// Helper function for [`Self::scaled_to`].
    #[must_use]
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn scale_point(point: (u32, u32), scale_factor: f64) -> (u32, u32) {
        (
            (f64::from(point.0) * scale_factor).floor() as u32,
            (f64::from(point.1) * scale_factor).floor() as u32,
        )
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

    /// Returns the delay in milliseconds.
    #[must_use]
    pub const fn delay(&self) -> u32 {
        self.delay
    }

    /// Returns a reference to the stored RGBA.
    #[must_use]
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    /// Validates a frame sequence and clears the delay for a static cursor.
    pub(super) fn validate_frames(frames: &mut [Self]) -> Result<()> {
        let Some((first, rest)) = frames.split_first_mut() else {
            bail!("cursor frame sequence is empty");
        };

        if rest.is_empty() {
            first.delay = 0;
            return Ok(());
        }

        let expected_dims = first.dimensions();

        for image in frames {
            if image.dimensions() != expected_dims {
                bail!("cursor frame sequence has inconsistent image dimensions");
            }

            if image.delay == 0 {
                bail!("animated cursors can't have frames with zero delay");
            }
        }

        Ok(())
    }
}

// skip rgba when debugging
impl fmt::Debug for CursorImage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CursorImage")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("hotspot_x", &self.hotspot_x)
            .field("hotspot_y", &self.hotspot_y)
            .field("delay", &self.delay)
            .finish_non_exhaustive()
    }
}
