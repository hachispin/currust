//! This is cursed af. Basically C at this point.
//!
//! Nearly everything here is `unsafe` so...
//!
//! Use [this](https://man.archlinux.org/man/Xcursor.3.en)
//! as your lifeline. Good luck :)
//!
//! Basic flow for each Xcursor:
//!
//! 1) Create [`XcursorImage`] structs.
//! 2) Bundle them into an [`XcursorImages`] struct.
//! 3) Save them using [`XcursorFileSaveImages`].
//! 4) Cleanup with [`XcursorImagesDestroy`].

use crate::cursors::common::CursorImage;

use std::{ffi::CString, path::Path};

use anyhow::{Context, Result, bail};
use libc::{fclose, fopen};
use x11::xcursor::{
    XcursorFileSaveImages, XcursorImage, XcursorImageCreate, XcursorImages, XcursorImagesCreate,
    XcursorImagesDestroy,
};

/// A delay value of zero is used for static (i.e, non-animated) Xcursors.
const STATIC_DELAY: u32 = 0;

/// Formula used for pre-multiplying a color channel with an alpha channel.
#[allow(clippy::cast_possible_truncation)]
#[inline]
const fn pre_alpha_formula(color: u32, alpha: u32) -> u8 {
    // +128 rounds to closest integer instead of floor
    ((color * alpha + 128) / 255) as u8
}

/// Converts RGBA packed pixels to pre-multipled ARGB.
///
/// ## Panics
///
/// If `rgba.len()` is not a multiple of four.
#[allow(clippy::cast_possible_truncation)]
fn to_pre_argb(rgba: &[u8]) -> Vec<u8> {
    assert!(rgba.len().is_multiple_of(4));

    let mut argb = Vec::with_capacity(rgba.len());

    for pixel in rgba.as_chunks::<4>().0 {
        // prevent overflow
        let [r, g, b, a] = pixel.map(u32::from);

        argb.push(pixel[3]); // push alpha first for ARGB
        argb.extend([r, g, b].map(|c| pre_alpha_formula(c, a)));
    }

    argb
}

/// Converts `Vec<u8>` to `Vec<u32>` with big-endian.
///
/// ## Panics
///
/// If `u8_vec.len()` is not a multiple of four.
fn u8_to_u32(u8_vec: &[u8]) -> Vec<u32> {
    assert!(
        u8_vec.len().is_multiple_of(4),
        "u8_vec length must be a multiple of four for conversion to `Vec<u32>`"
    );

    let mut u32_vec = Vec::with_capacity(u8_vec.len() / 4);

    for split_quad in u8_vec.as_chunks::<4>().0 {
        let quad = u32::from_be_bytes(*split_quad);
        u32_vec.push(quad);
    }

    u32_vec
}

/// Constructs an [`XcursorImage`] using `cursor`.
///
/// ## Errors
///
/// If [`XcursorImageCreate`] returns `NULL`.
unsafe fn construct_images(cursor: &CursorImage) -> Result<*mut XcursorImage> {
    let pixels = u8_to_u32(&to_pre_argb(cursor.rgba()));
    let dims = cursor.dimensions();

    let (width_i32, height_i32) = (dims.0.try_into()?, dims.1.try_into()?);
    let (xhot, yhot) = cursor.hotspot();
    let nominal_size = dims.0.max(dims.1);

    // `XcursorImageCreate()` allocates the `pixels` field and sets width, height
    let image = unsafe { XcursorImageCreate(height_i32, width_i32) };

    if image.is_null() {
        bail!("`XcursorImageCreate()` returned null");
    }

    // set fields
    unsafe {
        (*image).size = nominal_size;
        // (*image).width = cursor.width;    these should be set,
        // (*image).height = cursor.height;  otherwise... X_X
        (*image).xhot = xhot;
        (*image).yhot = yhot;
        (*image).delay = STATIC_DELAY;

        let num_pixels: usize = (dims.0 * dims.1).try_into()?;
        std::ptr::copy_nonoverlapping(pixels.as_ptr(), (*image).pixels, num_pixels);
    }

    Ok(image) // this isn't dangling trust me
}

/// Takes an array of [`XcursorImage`], grouping them as [`XcursorImages`].
///
/// NOTE: The returned [`XcursorImages`] must be freed with [`XcursorImagesDestroy`].
/// The stored images are freed by this function, so there's no need to manage that.
///
/// ## Errors
///
/// If [`XcursorImagesCreate`] returns `NULL`.
unsafe fn bundle_images(
    images: *mut *mut XcursorImage,
    num_images: usize,
) -> Result<*mut XcursorImages> {
    let num_images_i32: i32 = num_images.try_into()?;
    let xcur_images = unsafe { XcursorImagesCreate(num_images_i32) };

    if xcur_images.is_null() {
        bail!("`XcursorImagesCreate()` returned null");
    }

    unsafe {
        // `name` is only used for loading xcursor
        // from themes. we aren't doing that so...
        (*xcur_images).name = std::ptr::null_mut();
        (*xcur_images).nimage = num_images_i32;

        std::ptr::copy_nonoverlapping(images, (*xcur_images).images, num_images);
    }

    Ok(xcur_images)
}

/// Alias for `std::io::Error::last_os_error()`
fn errno() -> std::io::Error {
    std::io::Error::last_os_error()
}

/// Writes `images` as an Xcursor file to `path`.
///
/// ## Errors
///
/// - If [`fopen`]/[`fclose`] fails (returns non-zero)
/// - If [`XcursorFileSaveImages`] fails (returns zero)
///
/// [`errno`] is read upon failure and displayed in [`bail`] messages.
unsafe fn save_images(path: &str, images: *const XcursorImages) -> Result<()> {
    let path_c = CString::new(path)
        .with_context(|| format!("failed to create `CString` for path={path}"))?;

    let mode = CString::new("wb").unwrap();
    let file = unsafe { fopen(path_c.as_ptr(), mode.as_ptr()) };

    if file.is_null() {
        let err = errno();
        bail!("`fopen()` failed for path={path}: errno={err}");
    }

    let result = unsafe { XcursorFileSaveImages(file, images) };

    // xcursorlib uses 0 as error state
    if result == 0 {
        // we're already failing so it's
        // not like it can get any worse...
        let _ = unsafe { fclose(file) };
        let err = errno();
        bail!("`XcursorFileSaveImages()` failed: errno={err}");
    }

    let result = unsafe { fclose(file) };

    if result != 0 {
        let err = errno();
        bail!("`fclose()` failed: errno={err}");
    }

    Ok(())
}

/// Saves `cursor` to `path`.
///
/// ## Errors
///
/// If `path` has no `&str` representation, or errors
/// from the `unsafe` helper functions are propagated.
pub fn save_as_xcursor<P: AsRef<Path>>(cursor: &[CursorImage], path: P) -> Result<()> {
    let path = path.as_ref();

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
        save_images(path_str, images)?;
        XcursorImagesDestroy(images);
    };

    Ok(())
}
