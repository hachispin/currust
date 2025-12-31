//! Contains the [`CursorImage`] struct.

use crate::scaling::{scale_box_average, scale_nearest};

use anyhow::{Result, bail};

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
        // could be an if statement but whatever
        match scale_type {
            ScalingType::Upscale if scale_factor > Self::MAX_UPSCALE_FACTOR => {
                bail!(
                    "scale_factor={scale_factor} can't be greater than MAX_UPSCALE_FACTOR={}",
                    Self::MAX_UPSCALE_FACTOR
                );
            }

            ScalingType::Downscale if scale_factor > Self::MAX_DOWNSCALE_FACTOR => {
                bail!(
                    "scale_factor={scale_factor} can't be greater than MAX_DOWNSCALE_FACTOR={}",
                    Self::MAX_DOWNSCALE_FACTOR
                )
            }

            _ => (),
        }

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
    pub fn delay(&self) -> u32 {
        self.delay
    }

    /// Returns a reference to the stored RGBA.
    #[must_use]
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}
