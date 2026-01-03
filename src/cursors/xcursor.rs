//! Contains `unsafe` code for handling Xcursor.
//!
//! Basic flow for each Xcursor:
//!
//! 1) Create [`XcursorImage`] structs.
//! 2) Bundle them into an [`XcursorImages`] struct.
//! 3) Save them using [`XcursorFileSaveImages`].
//! 4) Cleanup with [`XcursorImagesDestroy`].
//!
//! You can [`man xcursor`](https://manpages.ubuntu.com/manpages/plucky/man3/Xcursor.3.html)
//! for documentation regarding any C functions.
//!
//! NOTE: rely on the `man` pages for parameter ordering.

/* TODO: replace with pure `binrw` structs */

use super::cursor_image::CursorImage;

use std::{
    ffi::{CStr, CString},
    ptr::NonNull,
};

use anyhow::{Context, Result, anyhow, bail};
use libc::{fclose, fopen, free};
use x11::xcursor::{
    XcursorFileSaveImages, XcursorImage, XcursorImageCreate, XcursorImageDestroy, XcursorImages,
    XcursorImagesCreate,
};

/// Macro for converting `ptr` (1st param) to [`NonNull<T>`], propagating
/// with [`anyhow::anyhow`] with the given `msg` (2nd param) if null.
///
/// This works with format strings.
macro_rules! denullify {
    ($ptr:expr, $($msg:tt)*) => {
        NonNull::new($ptr).ok_or_else(|| anyhow!($($msg)*))?
    };
}

/// Formula used for pre-multiplying a color channel with an alpha channel.
#[allow(clippy::cast_possible_truncation)]
#[inline]
const fn pre_alpha_formula(color: u32, alpha: u32) -> u8 {
    // +128 rounds to closest integer instead of floor
    ((color * alpha + 128) / 255) as u8
}

#[inline]
fn last_os_error() -> std::io::Error {
    std::io::Error::last_os_error()
}

/// [`XcursorImage`] with an implemented [`Drop`] trait
/// that calls [`XcursorImageDestroy`] for RAII.
///
/// This struct is [transparent](https://doc.rust-lang.org/nomicon/other-reprs.html#reprtransparent)
/// to allow for safe casts to `*mut XcursorImage` when needed.
///
/// ## Safety
///
/// For [`XcursorImages`], **do not call** `XcursorImagesDestroy`,
/// as this double-frees the pointers in `images`.
///
/// You can see this in the
/// [source code](https://gitlab.freedesktop.org/xorg/lib/libxcursor/-/blob/master/src/file.c#L133-L149).
#[repr(transparent)]
pub(super) struct XcursorImageHandle {
    /// This is not guaranteed to be valid unless it
    /// comes from a valid [`bundle_images`] call.
    inner: NonNull<XcursorImage>,
}

impl XcursorImageHandle {
    /// Equivalent to `self.inner.as_ptr()`.
    const fn as_ptr(&self) -> *mut XcursorImage {
        self.inner.as_ptr()
    }
}

impl Drop for XcursorImageHandle {
    fn drop(&mut self) {
        unsafe {
            XcursorImageDestroy(self.as_ptr());
        }
    }
}

impl From<NonNull<XcursorImage>> for XcursorImageHandle {
    fn from(non_null_image: NonNull<XcursorImage>) -> Self {
        Self {
            inner: non_null_image,
        }
    }
}

/// RAII wrapper around [`XcursorImages`].
///
/// This struct is [transparent](https://doc.rust-lang.org/nomicon/other-reprs.html#reprtransparent)
/// to allow for safe casts to `*mut XcursorImages` when needed.
///
/// ## Safety
///
/// This does not call `XcursorImagesDestroy` because
/// doing so would double-free the `images` pointers.
///
/// Images are already memory managed in [`XcursorImageHandle`].
/// **Do not pass** non-managed [`XcursorImage`]'s here, it will leak!
#[repr(transparent)]
pub(super) struct XcursorImagesHandle {
    /// The stored images here must be managed by [`XcursorImageHandle`].
    ///
    /// They are **not** freed by the [`Drop`] trait for this struct.
    inner: NonNull<XcursorImages>,
}

impl XcursorImagesHandle {
    const fn as_ptr(&self) -> *mut XcursorImages {
        self.inner.as_ptr()
    }
}

impl Drop for XcursorImagesHandle {
    fn drop(&mut self) {
        unsafe {
            let raw = self.as_ptr();
            let name = (*raw).name;
            free(name.cast());
            free(raw.cast());
        }
    }
}

impl From<NonNull<XcursorImages>> for XcursorImagesHandle {
    fn from(non_null_images: NonNull<XcursorImages>) -> Self {
        Self {
            inner: non_null_images,
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

    u8_vec
        .as_chunks::<4>()
        .0 // ignore remainder
        .iter()
        .map(|&q| u32::from_be_bytes(q))
        .collect()
}

/// Constructs an [`XcursorImageHandle`] using `cursor`.
///
/// This is not marked `unsafe` as the caller cannot cause UB.
///
/// ## Errors
///
/// - If [`XcursorImageCreate`] returns `NULL`.
/// - If [`TryInto`] conversions fail.
pub(super) fn construct_images(cursor: &CursorImage) -> Result<XcursorImageHandle> {
    let pixels = u8_to_u32(&to_pre_argb(cursor.rgba()));
    let dims = cursor.dimensions();

    let (width_i32, height_i32) = (dims.0.try_into()?, dims.1.try_into()?);
    let (xhot, yhot) = cursor.hotspot();
    let nominal_size = dims.0.max(dims.1);

    // `XcursorImageCreate()` allocates the `pixels` field and sets width, height
    let image = unsafe { XcursorImageCreate(width_i32, height_i32) };
    let mut image = denullify!(image, "`XcursorImageCreate()` returned null");

    // set fields
    let num_pixels: usize = (dims
        .0
        .checked_mul(dims.1)
        .ok_or_else(|| anyhow!("overflow on dims product")))?
    .try_into()?;

    let image_mut = unsafe { image.as_mut() };

    image_mut.size = nominal_size;
    image_mut.xhot = xhot;
    image_mut.yhot = yhot;
    image_mut.delay = cursor.delay();

    unsafe {
        std::ptr::copy_nonoverlapping(pixels.as_ptr(), image_mut.pixels, num_pixels);
    }

    Ok(image.into())
}

/// Takes an array of [`XcursorImageHandle`], grouping them as [`XcursorImages`].
///
/// NOTE: The returned [`XcursorImages`] inherently has RAII from
/// [`XcursorImageHandle`], so you don't need to do any manual cleanup.
///
/// ## Safety
///
/// `images`'s elements are assumed to be valid
/// and constructed by [`construct_images`].
///
/// ## Errors
///
/// - If [`XcursorImagesCreate`] returns `NULL`, or if [`TryInto`] conversions fail.
/// - If `images` is empty.
pub(super) unsafe fn bundle_images(
    images: &mut [XcursorImageHandle],
) -> Result<XcursorImagesHandle> {
    if images.is_empty() {
        bail!("`images` can't be empty");
    }

    let num_images_i32: i32 = images.len().try_into()?;

    let xcur_images = unsafe { XcursorImagesCreate(num_images_i32) };
    let mut xcur_images = denullify!(xcur_images, "`XcursorImagesCreate() returned null`");
    let xcur_images_mut = unsafe { xcur_images.as_mut() };

    // `name` is only used for loading xcursor
    // from themes. we aren't doing that so...
    xcur_images_mut.name = std::ptr::null_mut();
    xcur_images_mut.nimage = num_images_i32;

    // cast *mut XcursorImageHandle => *mut *mut XcursorImage
    // this is safe because XcursorImageHandle is transparent
    let images_raw: *mut *mut XcursorImage = images.as_mut_ptr().cast();

    unsafe {
        std::ptr::copy_nonoverlapping(images_raw, xcur_images_mut.images, images.len());
    }

    Ok(xcur_images.into())
}

/// Writes `images` as an Xcursor file to `path`.
///
/// NOTE: This is not atomic, so partially-written 
///       files may be created if saving fails.
///
/// ## Safety
///
/// `images` is assumed to be valid and constructed
/// by a valid (safe) [`bundle_images`] call.
///
/// ## Errors
///
/// - If `path` can't be converted to [`CString`]
/// - If [`fopen`]/[`fclose`] fails (returns non-zero)
/// - If [`XcursorFileSaveImages`] fails (returns zero)
///
/// [`last_os_error`] is read upon failure and displayed in [`bail`] messages
/// but it's not reset beforehand, so you may get unrelated errors.
pub(super) unsafe fn save_images(path: &str, images: &XcursorImagesHandle) -> Result<()> {
    const WRITE_BINARY: &CStr = c"wb";

    if path.is_empty() {
        bail!("`path` can't be empty");
    }

    let path_c = CString::new(path)
        .with_context(|| format!("failed to create `CString` for path={path}"))?;

    let file = unsafe { fopen(path_c.as_ptr(), WRITE_BINARY.as_ptr()) };
    let file = denullify!(
        file,
        "`fopen()` failed for path={path}: errno={}",
        last_os_error()
    );
    let file_ptr = file.as_ptr();

    // this is not atomic (from testing), so you
    // may end up with partially-written files :(
    let result = unsafe { XcursorFileSaveImages(file_ptr, images.as_ptr()) };

    // libXcursor uses 0 as error state
    if result == 0 {
        // we're already failing so it's not like it can get any worse...
        let _ = unsafe { fclose(file_ptr) };
        bail!(
            "`XcursorFileSaveImages()` failed: errno={}",
            last_os_error()
        );
    }

    let result = unsafe { fclose(file_ptr) };

    if result != 0 {
        bail!("`fclose()` failed: errno={}", last_os_error());
    }

    Ok(())
}
