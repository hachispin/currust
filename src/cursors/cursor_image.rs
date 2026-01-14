//! Contains the [`CursorImage`] struct.

use crate::scaling::{scale_box_average, scale_nearest};

use core::fmt;

use anyhow::{Context, Result, bail};
use ico::{IconDirEntry, ResourceType};

/// Used in scaling functions.
#[derive(Debug, Clone, Copy)]
#[allow(missing_docs)]
pub enum ScalingType {
    Upscale,
    Downscale,
}

/// Represents a generic cursor *image*.
///
/// An actual cursor is usually expressed as a
/// vector of cursor images. See [`GenericCursor`].
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

    /// Returns a new [`CursorImage`] scaled up/down to `scale_factor`.
    ///
    /// - Upscaling uses [nearest-neighbour](https://en.wikipedia.org/wiki/Image_scaling#Nearest-neighbor_interpolation).
    /// - Downscaling uses [box averaging](https://en.wikipedia.org/wiki/Image_scaling#Box_sampling).
    ///
    /// ## Errors
    ///
    /// If `scale_factor` is greater than [`Self::MAX_UPSCALE_FACTOR`]
    /// or [`Self::MAX_DOWNSCALE_FACTOR`], depending on `scaling_type`.
    pub fn scaled_to(&self, scale_factor: u32, scale_type: ScalingType) -> Result<Self> {
        let (width, height) = self.dimensions();
        let (scaled_width, scaled_height) = match scale_type {
            ScalingType::Upscale => (width * scale_factor, height * scale_factor),
            ScalingType::Downscale => (width / scale_factor, height / scale_factor),
        };

        let scaling_algorithm = match scale_type {
            ScalingType::Upscale => scale_nearest,
            ScalingType::Downscale => scale_box_average,
        };

        let scaled_rgba =
            scaling_algorithm(self.rgba(), width, height, scaled_width, scaled_height);

        let (hotspot_x, hotspot_y) = self.hotspot();
        let (scaled_hotspot_x, scaled_hotspot_y) = match scale_type {
            ScalingType::Upscale => (hotspot_x * scale_factor, hotspot_y * scale_factor),
            ScalingType::Downscale => (hotspot_x / scale_factor, hotspot_y / scale_factor),
        };

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

    /// Returns the max of width and height.
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
    /// Returns a reference to the first stored element in [`Self::inner`].
    #[inline]
    pub fn first(&self) -> &CursorImage {
        &self.inner[0]
    }

    /// Equivalent to `self.inner.len()`.
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
            bail!("can't create CursorImages from empty vec, call new() instead");
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
