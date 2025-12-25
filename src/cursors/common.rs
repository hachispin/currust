//! Contains the [`GenericCursor`] struct, which is used
//! as a medium between Windows and Linux cursors.

use super::xcursor::{bundle_images, construct_images, save_images};

use std::{fs::File, path::Path};

use anyhow::{Context, Result, bail};
use ico::IconDir;

/// Represents a generic cursor *image*.
///
/// An actual cursor is usually expressed as a
/// vector of cursor images. See [`GenericCursor`].
#[derive(Debug)]
pub struct CursorImage {
    width: u32,
    height: u32,
    hotspot_x: u32,
    hotspot_y: u32,
    rgba: Vec<u8>,
}

impl CursorImage {
    /// The max upscaling factor for images.
    pub const MAX_UPSCALE_FACTOR: u32 = 20;
    /// The max downscaling factor for images.
    pub const MAX_DOWNSCALE_FACTOR: u32 = 5;

    /// Contructor for [`CursorImage`].
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
        })
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
    images: Vec<CursorImage>,
}

impl GenericCursor {
    /// Trivial constructor.
    ///
    /// ## Errors
    ///
    /// If any image in `images` has the same nominal
    /// size as another image, or if `images` is empty.
    pub fn new(images: Vec<CursorImage>) -> Result<Self> {
        if images.is_empty() {
            bail!("`images` can't be empty");
        }

        let mut seen_nominal_sizes = Vec::with_capacity(images.len());

        // no hashset because small collection
        for image in &images {
            let dims = image.dimensions();
            let nominal_size = dims.0.max(dims.1);

            if seen_nominal_sizes.contains(&nominal_size) {
                bail!(
                    "`GenericCursor` can't be constructed \
                    with images that have the same nominal size"
                );
            }

            seen_nominal_sizes.push(nominal_size);
        }

        Ok(Self { images })
    }

    /// Helper function for [`Self::add_scale`].
    fn has_nominal_size(&self, nominal_size: u32) -> bool {
        for image in &self.images {
            let dims = image.dimensions();

            if dims.0.max(dims.1) == nominal_size {
                return true;
            }
        }

        false
    }

    /// Adds an *upscaled* [`CursorImage`] to [`Self::images`]. This
    /// scales based off of the first element in [`Self::images`].
    ///
    /// ## Errors
    ///
    /// If the newly made [`CursorImage`] doesn't
    /// have a unique nominal size.
    pub fn add_upscale(&mut self, scale_factor: u32) -> Result<()> {
        // this won't panic because new() guarantees at least 1
        let base = &self.images[0];
        let dims = base.dimensions();
        let scaled_dims = (dims.0 * scale_factor, dims.1 * scale_factor);
        let scaled_nominal = scaled_dims.0.max(scaled_dims.1);

        if self.has_nominal_size(scaled_nominal) {
            bail!("duplicate nominal size");
        }

        let scaled_image = base.upscaled_to(scale_factor)?;
        self.images.push(scaled_image);

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
        // this won't panic because new() guarantees at least 1
        let base = &self.images[0];
        let dims = base.dimensions();
        let scaled_dims = (dims.0 / scale_factor, dims.1 / scale_factor);
        let scaled_nominal = scaled_dims.0.max(scaled_dims.1);

        if self.has_nominal_size(scaled_nominal) {
            bail!("duplicate nominal size");
        }

        let scaled_image = base.downscaled_to(scale_factor)?;
        self.images.push(scaled_image);

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
        let mut images = Vec::with_capacity(entries.len());

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
        let cursor = self.images.as_slice();

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

    #[must_use]
    /// Trivial accessor for `images` field.
    pub fn images(&self) -> &[CursorImage] {
        &self.images
    }
}

impl From<CursorImage> for GenericCursor {
    fn from(image: CursorImage) -> Self {
        Self {
            images: vec![image],
        }
    }
}

/// Nearest-neighbour scaling algorithm for RGBA8.
///
/// This is center-aligned and used for *upscaling*.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn scale_nearest(src: &[u8], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u8> {
    let mut dst = vec![0u8; (dst_w * dst_h * 4) as usize];

    let scale_x = src_w as f32 / dst_w as f32;
    let scale_y = src_h as f32 / dst_h as f32;

    // lowkenuinely copied from chatgpt so i have
    // no clue what this does. it works though
    for y in 0..dst_h {
        for x in 0..dst_w {
            let src_x = ((x as f32 + 0.5) * scale_x - 0.5)
                .round()
                .clamp(0.0, (src_w - 1) as f32) as u32;

            let src_y = ((y as f32 + 0.5) * scale_y - 0.5)
                .round()
                .clamp(0.0, (src_h - 1) as f32) as u32;

            let src_idx = ((src_y * src_w + src_x) * 4) as usize;
            let dst_idx = ((y * dst_w + x) * 4) as usize;

            dst[dst_idx..dst_idx + 4].copy_from_slice(&src[src_idx..src_idx + 4]);
        }
    }

    dst
}

/// Box sampling/averaging algorithm for RGBA8.
///
/// This is used for *downscaling*.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn scale_box_average(src: &[u8], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u8> {
    let mut dst = vec![0u8; (dst_w * dst_h * 4) as usize];

    let scale_x = src_w as f32 / dst_w as f32;
    let scale_y = src_h as f32 / dst_h as f32;

    // same thing with this. just copied so i'm
    // clueles on what this actually does
    for y in 0..dst_h {
        for x in 0..dst_w {
            // find the source rectangle this dst pixel covers
            let x0 = (x as f32 * scale_x).floor() as u32;
            let y0 = (y as f32 * scale_y).floor() as u32;
            let x1 = ((x + 1) as f32 * scale_x).ceil().min(src_w as f32) as u32;
            let y1 = ((y + 1) as f32 * scale_y).ceil().min(src_h as f32) as u32;

            let mut r_sum = 0u32;
            let mut g_sum = 0u32;
            let mut b_sum = 0u32;
            let mut a_sum = 0u32;
            let mut count = 0u32;

            for sy in y0..y1 {
                for sx in x0..x1 {
                    let idx = ((sy * src_w + sx) * 4) as usize;
                    r_sum += u32::from(src[idx]);
                    g_sum += u32::from(src[idx + 1]);
                    b_sum += u32::from(src[idx + 2]);
                    a_sum += u32::from(src[idx + 3]);
                    count += 1;
                }
            }

            let dst_idx = ((y * dst_w + x) * 4) as usize;
            dst[dst_idx] = (r_sum / count) as u8;
            dst[dst_idx + 1] = (g_sum / count) as u8;
            dst[dst_idx + 2] = (b_sum / count) as u8;
            dst[dst_idx + 3] = (a_sum / count) as u8;
        }
    }

    dst
}
