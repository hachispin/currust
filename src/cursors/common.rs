//! Contains the [`GenericCursor`] struct, which is used
//! as a medium between Windows and Linux cursors.

use super::xcursor::{bundle_images, construct_images, save_images};

use std::{fs::File, path::Path};

use anyhow::{Context, Result, bail};
use ico::IconDir;
use x11::xcursor::XcursorImagesDestroy;

/// Represents a generic cursor *image*.
///
/// An actual cursor is usually expressed as a
/// vector of cursor images. See [`GenericCursor`].
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
        if hotspot_x > width {
            bail!("hotspot_x={hotspot_x} cannot be greater than width={width}");
        }

        if hotspot_y > height {
            bail!("hotspot_y={hotspot_y} cannot be greater than height={height}");
        }

        if (width * height * 4) != rgba.len().try_into()? {
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
    pub const fn rgba(&self) -> &Vec<u8> {
        &self.rgba
    }
}

/// Represents a generic cursor.
///
/// `images` is guaranteed to not have any images
/// that share the same dimensions.
pub struct GenericCursor {
    images: Vec<CursorImage>,
}

impl GenericCursor {
    /// Trivial constructor.
    ///
    /// ## Errors
    ///
    /// If any image in `images` has the same image
    /// dimensions as another image.
    pub fn new(images: Vec<CursorImage>) -> Result<Self> {
        let mut seen_dims = Vec::with_capacity(images.len());

        // no hashset because small collection
        for image in &images {
            let dims = image.dimensions();

            if seen_dims.contains(&dims) {
                bail!(
                    "`GenericCursor` can't be constructed \
                    with images that have the same dimensions"
                );
            }

            seen_dims.push(dims);
        }

        Ok(Self { images })
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
            let hotspot = image.cursor_hotspot().ok_or(anyhow::anyhow!(
                "provided cur_path={cur_path_display} must be to CUR, not ICO"
            ))?;

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

        let path_str = path.to_str().ok_or(anyhow::anyhow!(
            "failed to convert path={} to &str",
            path.display()
        ))?;

        let mut images_vec = Vec::with_capacity(cursor.len());

        for c in cursor {
            let image = unsafe { construct_images(c)? };
            images_vec.push(image);
        }

        // `images_vec` must not realloc after this or UB happens
        let images_ptr = images_vec.as_mut_ptr();
        let images = unsafe { bundle_images(images_ptr, cursor.len())? };

        unsafe {
            let res = save_images(path_str, images);
            XcursorImagesDestroy(images);
            res?;
        };

        Ok(())
    }
}

impl From<CursorImage> for GenericCursor {
    fn from(image: CursorImage) -> Self {
        Self {
            images: vec![image],
        }
    }
}
