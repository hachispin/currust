//! Contains the [`GenericCursor`] struct.

use crate::cursors::ani::AniFile;

use super::{
    cursor_image::{CursorImage, ScalingType},
    xcursor::{bundle_images, construct_images, save_images},
};

use std::{
    fs::{self, File},
    io::Cursor,
    path::Path,
};

use anyhow::{Context, Result, bail};
use ico::IconDir;

/// Represents a generic cursor.
///
/// `images` is guaranteed to not have any images
/// that share the same nominal sizes.
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
            scale_factors: vec![1.0],
        })
    }

    /// Adds scaled [`CursorImage`] from `base` to `scaled`.
    ///
    /// NOTE: Downscaled images isn't recommended.
    ///
    /// ## Errors
    ///
    /// If the newly made [`CursorImage`] doesn't
    /// have a unique nominal size.
    pub fn add_scale(&mut self, scale_factor: u32, scale_type: ScalingType) -> Result<()> {

        let canon_scale_factor: f64 = match scale_type {
            ScalingType::Upscale => f64::from(scale_factor),
            ScalingType::Downscale => 1.0 / f64::from(scale_factor),
        };

        if self.scale_factors.contains(&canon_scale_factor) {
            bail!("scale_factor={scale_factor} already added");
        }

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

    /// Parses `ani_path`.
    ///
    /// ## Errors
    ///
    /// Path `ani_path` is not to a valid ANI cursor.
    pub fn from_ani_path<P: AsRef<Path>>(ani_path: P) -> Result<Self> {
        let ani_path = ani_path.as_ref();
        let ani_path_display = ani_path.display();

        if ani_path.extension().is_none_or(|ext| ext != "ani") {
            bail!("expected {ani_path_display} to have extension 'ani'")
        }

        let ani_blob = fs::read(ani_path)?;
        let ani_file = AniFile::from_blob(&ani_blob)?;
        println!("{ani_file:#?}");
        let header = ani_file.header;

        let icos: Vec<IconDir> = ani_file
            .ico_frames
            .into_iter()
            .map(|chunk| IconDir::read(&mut Cursor::new(&chunk.data)))
            .collect::<Result<_, _>>()?;

        let num_steps = usize::try_from(header.num_steps)?;
        let delays_jiffies = ani_file
            .sequence
            .map_or(vec![header.jiffy_rate; num_steps], |chunk| chunk.data);

        // jiffies are 1/60th of a second
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let delays_ms: Vec<u32> = delays_jiffies
            .into_iter()
            .map(|j| (f64::from(j) * 1000.0 / 60.0).round() as u32)
            .collect();

        let mut canon_entries = Vec::with_capacity(icos.len());

        // using for loop for easier inspection
        for ico in &icos {
            let entries = ico.entries();

            /* TODO: find a better way to handle >1 entries here */
            match entries.len() {
                0 => {
                    eprintln!("Warning: skipping IconDir with 0 entries (ANI)");
                }

                1 => canon_entries.push(entries[0].clone()),

                _ => {
                    eprintln!("Warning: found multiple entries, only parsing first (ANI)");
                    canon_entries.push(entries[0].clone());
                }
            }
        }

        // assert_eq!(canon_entries.len(), delays_ms.len());

        /* TODO: handle custom sequences */
        let mut cursor_images = Vec::with_capacity(canon_entries.len());
        for (entry, delay) in canon_entries.into_iter().zip(delays_ms) {
            let (hotspot_x, hotspot_y) = entry.cursor_hotspot().unwrap();
            let rgba = entry.decode()?.into_rgba_data();

            let cursor_image = CursorImage::new_with_delay(
                entry.width(),
                entry.height(),
                hotspot_x.into(),
                hotspot_y.into(),
                rgba,
                delay,
            )?;

            cursor_images.push(cursor_image);
        }

        GenericCursor::new(cursor_images)
    }

    /// Saves `cursor` to `path` in Xcursor format.
    ///
    /// ## Errors
    ///
    /// If `path` has no `&str` representation, or errors
    /// from the `unsafe` helper functions are propagated.
    pub fn save_as_xcursor<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        let joined: Vec<CursorImage> = self.joined_images().cloned().collect();
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
