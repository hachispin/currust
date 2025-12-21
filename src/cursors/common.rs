//! Contains the [`CursorImage`] struct, which is used
//! as a medium between Windows and Linux cursors.

use anyhow::{Result, bail};

/// Represents a generic cursor image.
///
/// An actual cursor is usually expressed
/// as a vector of cursor images.
pub struct CursorImage {
    width: u32,
    height: u32,
    hotspot_x: u32,
    hotspot_y: u32,
    rgba: Vec<u8>,
}

impl CursorImage {
    /// Contructor for [`CursorImage`].
    ///
    /// ## Panics
    ///
    /// If `rgba.len()` can't be expressed as `u32`
    /// without reinterpreting.
    ///
    /// ## Errors
    ///
    /// - If `hotspot_x > width` and ditto for y.
    /// - If `width * height * 4 != rgba.len()`.
    pub fn new(
        width: u32,
        height: u32,
        hotspot_x: u32,
        hotspot_y: u32,
        rgba: Vec<u8>,
    ) -> Result<Self> {
        if hotspot_x > width {
            bail!("hotspot_x={hotspot_x} cannot be greater than width={width}");
        }

        if hotspot_y > height {
            bail!("hotspot_y={hotspot_y} cannot be greater than height={height}");
        }

        if (width * height * 4) != rgba.len().try_into().unwrap() {
            bail!(
                "expected rgba.len()={}, instead got rgba.len()={}",
                width * height * 4,
                rgba.len()
            );
        }

        if width != height {
            eprintln!(
                "width={width} and height={height} aren't equal. \
                this may cause odd behaviour"
            );
        }

        Ok(Self {
            width,
            height,
            hotspot_x,
            hotspot_y,
            rgba,
        })
    }

    /// Returns image dimensions as [width, height].
    #[must_use] 
    pub const fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Returns hotspot coordinates as [x, y].
    #[must_use] 
    pub const fn hotspot(&self) -> (u32, u32) {
        (self.hotspot_x, self.hotspot_y)
    }

    /// Returns a reference to the stored RGBA.
    #[must_use] 
    pub const fn rgba(&self) -> &Vec<u8> {
        &self.rgba
    }
}
