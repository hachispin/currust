//! Contains `unsafe` code for handling Xcursor.
//!
//! Basic flow for each Xcursor:
//!
//! 1) Create [`XcursorImage`] structs.
//! 2) Bundle them into an [`XcursorImages`] struct.
//! 3) Save them using [`XcursorFileSaveImages`].
//! 4) Cleanup with [`XcursorImagesDestroy`].
//!
//! You can `man xcursor` to read documentation
//! for the exposed C functions (from xcursorlib).
//!
//! Rely on the `man` pages for parameter ordering.

use crate::cursors::common::CursorImage;

use std::{
    ffi::{CStr, CString},
    ptr::NonNull,
};

use anyhow::{Context, Result, bail};
use libc::{fclose, fopen};
use x11::xcursor::{
    XcursorFileSaveImages, XcursorImage, XcursorImageCreate, XcursorImages, XcursorImagesCreate,
    XcursorImagesDestroy,
};

/// Macro for converting `ptr` (1st param) to [`NonNull<T>`], propagating
/// with [`anyhow::anyhow`] with the given `msg` (2nd param) if null.
///
/// This works with format strings.
macro_rules! denullify {
    ($ptr:expr, $($msg:tt)*) => {
        NonNull::new($ptr).ok_or_else(|| anyhow::anyhow!($($msg)*))?
    };
}

/// A delay value of zero is used for static (i.e, non-animated) Xcursors.
const STATIC_DELAY: u32 = 0;

/// Formula used for pre-multiplying a color channel with an alpha channel.
#[allow(clippy::cast_possible_truncation)]
#[inline]
const fn pre_alpha_formula(color: u32, alpha: u32) -> u8 {
    // +128 rounds to closest integer instead of floor
    ((color * alpha + 128) / 255) as u8
}

/// [`XcursorImages`] with [`Drop`] trait that calls
/// the [`XcursorImagesDestroy`] destructor for RAII.
pub(super) struct XcursorImagesHandle {
    inner: NonNull<XcursorImages>,
}

impl XcursorImagesHandle {
    // /// Constructor for `*mut XcursorImages`.
    // ///
    // /// Use the infallible [`From`] trait if you
    // /// already have a `NonNull<XcursorImages>`.
    // fn new(ptr: *mut XcursorImages) -> Option<Self> {
    //     NonNull::new(ptr).map(|inner| Self { inner })
    // }

    /// Equivalent to `self.inner.as_ptr()`.
    fn as_ptr(&self) -> *mut XcursorImages {
        self.inner.as_ptr()
    }
}

impl From<NonNull<XcursorImages>> for XcursorImagesHandle {
    fn from(non_null_images: NonNull<XcursorImages>) -> Self {
        Self {
            inner: non_null_images,
        }
    }
}

impl Drop for XcursorImagesHandle {
    fn drop(&mut self) {
        unsafe {
            XcursorImagesDestroy(self.inner.as_ptr());
        }
    }
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
pub(super) unsafe fn construct_images(cursor: &CursorImage) -> Result<NonNull<XcursorImage>> {
    let pixels = u8_to_u32(&to_pre_argb(cursor.rgba()));
    let dims = cursor.dimensions();

    let (width_i32, height_i32) = (dims.0.try_into()?, dims.1.try_into()?);
    let (xhot, yhot) = cursor.hotspot();
    let nominal_size = dims.0.max(dims.1);

    // `XcursorImageCreate()` allocates the `pixels` field and sets width, height
    let image = unsafe { XcursorImageCreate(width_i32, height_i32) };
    let mut image = denullify!(image, "`XcursorImageCreate()` returned null");

    // set fields
    let num_pixels: usize = (dims.0 * dims.1).try_into()?;
    let image_mut = unsafe { image.as_mut() };

    image_mut.size = nominal_size;
    image_mut.xhot = xhot;
    image_mut.yhot = yhot;
    image_mut.delay = STATIC_DELAY;

    unsafe {
        std::ptr::copy_nonoverlapping(pixels.as_ptr(), image_mut.pixels, num_pixels);
    }

    Ok(image) // this isn't dangling trust me
}

/// Takes an array of [`XcursorImage`], grouping them as [`XcursorImages`].
///
/// NOTE: The returned [`XcursorImagesHandle`] implements [`Drop`].
/// In other words, don't worry about it :)
///
/// ## Errors
///
/// If [`XcursorImagesCreate`] returns `NULL`, or if [`TryInto`] conversions fail.
pub(super) unsafe fn bundle_images(
    images: *mut *mut XcursorImage,
    num_images: usize,
) -> Result<XcursorImagesHandle> {
    let num_images_i32: i32 = num_images.try_into()?;
    let xcur_images = unsafe { XcursorImagesCreate(num_images_i32) };
    let mut xcur_images = denullify!(xcur_images, "`XcursorImagesCreate() returned null`");

    let xcur_images_mut = unsafe { xcur_images.as_mut() };
    // `name` is only used for loading xcursor
    // from themes. we aren't doing that so...
    xcur_images_mut.name = std::ptr::null_mut();
    xcur_images_mut.nimage = num_images_i32;

    unsafe {
        std::ptr::copy_nonoverlapping(images, xcur_images_mut.images, num_images);
    }

    Ok(xcur_images.into())
}

/// Alias for `std::io::Error::last_os_error()`
#[inline]
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
pub(super) unsafe fn save_images(path: &str, images: &XcursorImagesHandle) -> Result<()> {
    const WRITE_BINARY: &CStr = c"wb";

    let path_c = CString::new(path)
        .with_context(|| format!("failed to create `CString` for path={path}"))?;

    let file = unsafe { fopen(path_c.as_ptr(), WRITE_BINARY.as_ptr()) };
    let file = denullify!(file, "`fopen()` failed for path={path}: errno={}", errno());
    let file_ptr = file.as_ptr();
    let result = unsafe { XcursorFileSaveImages(file.as_ptr(), images.as_ptr()) };

    // xcursorlib uses 0 as error state
    if result == 0 {
        // we're already failing so it's not like it can get any worse...
        let _ = unsafe { fclose(file_ptr) };
        let err = errno();
        bail!("`XcursorFileSaveImages()` failed: errno={err}");
    }

    let result = unsafe { fclose(file_ptr) };

    if result != 0 {
        let err = errno();
        bail!("`fclose()` failed: errno={err}");
    }

    Ok(())
}
