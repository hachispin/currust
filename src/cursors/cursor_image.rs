//! Contains the [`CursorImage`] and [`CursorImages`] struct.
//!
//! These represents the frames of static/animated cursors.

use std::fmt;

use anyhow::{Context, Result, bail};
use fast_image_resize::{
    PixelType, ResizeAlg, ResizeOptions, Resizer,
    images::{Image, ImageRef},
};
use ico::{IconDirEntry, ResourceType};

/// Represents a generic cursor *image*.
#[derive(Clone)]
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
    pub fn from_entry(entry: &IconDirEntry, delay: u32) -> Result<Self> {
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
    pub fn scaled_to(&self, scale_factor: f64, algorithm: ResizeAlg) -> Result<Self> {
        let (w1, h1) = self.dimensions();
        let (w2, h2) = Self::scale_point((w1, h1), scale_factor);
        let (hx2, hy2) = Self::scale_point(self.hotspot(), scale_factor);
        let src = ImageRef::new(w1, h1, self.rgba(), PixelType::U8x4)?;
        let mut dst = Image::new(w2, h2, PixelType::U8x4);
        let mut resizer = Resizer::new();
        let options = ResizeOptions::new().resize_alg(algorithm);
        resizer.resize(&src, &mut dst, &options)?;

        Ok(Self {
            width: w2,
            height: h2,
            hotspot_x: hx2,
            hotspot_y: hy2,
            rgba: dst.into_vec(),
            delay: self.delay,
        })
    }

    /// Helper function for [`Self::scaled_to`].
    #[inline]
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn scale_point(point: (u32, u32), scale_factor: f64) -> (u32, u32) {
        (
            (f64::from(point.0) * scale_factor) as u32,
            (f64::from(point.1) * scale_factor) as u32,
        )
    }

    /// Returns image dimensions as (width, height).
    #[inline]
    #[must_use]
    pub const fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Returns hotspot coordinates as (x, y).
    #[inline]
    #[must_use]
    pub const fn hotspot(&self) -> (u32, u32) {
        (self.hotspot_x, self.hotspot_y)
    }

    /// Returns the delay in milliseconds.
    #[inline]
    #[must_use]
    pub const fn delay(&self) -> u32 {
        self.delay
    }

    /// Returns a reference to the stored RGBA.
    #[inline]
    #[must_use]
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    /// Returns the max of width and height.
    #[inline]
    #[must_use]
    pub fn nominal_size(&self) -> u32 {
        self.dimensions().0.max(self.dimensions().1)
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

/// Wrapper around [`Vec<CursorImage>`].
///
/// This represents a valid sequence of frames
/// or a single frame for static cursors.
///
/// Held invariants:
///
/// - There should be at least one frame.
/// - Each frame should share the same dimensions.
/// - If there is one frame, the delay of it is zero.
/// - If there are multiple frames, all delays are non-zero.
#[derive(Debug)]
pub(super) struct CursorImages {
    inner: Vec<CursorImage>,
}

impl CursorImages {
    /// Returns a reference to the first stored element in `inner`.
    #[inline]
    pub fn first(&self) -> &CursorImage {
        &self.inner[0]
    }

    /// Equivalent to `inner.len()`.
    #[inline]
    pub const fn len(&self) -> usize {
        self.inner.len()
    }

    /// Accessor for `inner`.
    #[inline]
    pub fn inner(&self) -> &[CursorImage] {
        &self.inner
    }
}

impl TryFrom<Vec<CursorImage>> for CursorImages {
    type Error = anyhow::Error;

    fn try_from(vec: Vec<CursorImage>) -> Result<Self> {
        if vec.is_empty() {
            bail!("can't create CursorImages from empty vec");
        }

        if vec.len() == 1 {
            if vec[0].delay != 0 {
                bail!("delay must be zero for CursorImages if there is only one image");
            }

            return Ok(Self { inner: vec });
        }

        let expected_dims = vec[0].dimensions();
        if vec.iter().any(|img| img.dimensions() != expected_dims) {
            bail!("can't create CursorImages with inconsistent image dimensions");
        }

        if vec.iter().any(|img| img.delay == 0) {
            bail!("animated cursors can't have frames with zero delay");
        }

        Ok(Self { inner: vec })
    }
}

impl From<CursorImages> for Vec<CursorImage> {
    fn from(images: CursorImages) -> Self {
        images.inner
    }
}
