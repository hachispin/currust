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

use crate::cursors::common::CursorImage;

use x11::xcursor::{XcursorFileSaveImages, XcursorImage, XcursorImages};

/// Both image and comment chunks use `version: 1`.
const CHUNK_VERSION: u32 = 1;

/// A delay value of zero is used for static (i.e, non-animated) Xcursors.
const STATIC_DELAY: u32 = 0;

/// Converts RGBA packed pixels to ARGB.
///
/// ## Panics
///
/// If `rgba.len()` is not a multiple of four.
fn to_argb(rgba: &[u8]) -> Vec<u8> {
    assert!(
        rgba.len().is_multiple_of(4),
        "rgba length must be a multiple of four"
    );

    let mut argb = Vec::with_capacity(rgba.len());

    for pixel in rgba.as_chunks::<4>().0 {
        //    RGBA    ->    ARGB
        // 0, 1, 2, 3 -> 3, 0, 1, 2
        argb.push(pixel[3]);
        argb.push(pixel[0]);
        argb.push(pixel[1]);
        argb.push(pixel[2]);
    }

    argb
}

/// Converts `Vec<u8>` to `Vec<u32>`
///
/// ## Panics
///
/// If `u8_vec.len()` is not a multiple of four.
fn u8_to_u32(u8_vec: Vec<u8>) -> Vec<u32> {
    assert!(
        u8_vec.len().is_multiple_of(4),
        "u8_vec length must be a multiple of four for conversion to `Vec<u32>`"
    );

    let mut u32_vec = Vec::with_capacity(u8_vec.len().div_ceil(4));

    for split_quad in u8_vec.as_chunks::<4>().0 {
        // x11 uses little-endian ordering
        let quad = u32::from_le_bytes(*split_quad);
        u32_vec.push(quad);
    }

    u32_vec
}

/// Converts the given `cursor` to [`_XcursorImage`].
///
/// NOTE: [`XcursorImage::pixels`] is heap-allocated!
unsafe fn pack(cursor: &CursorImage) -> XcursorImage {
    // xcursor wants an ARGB u32 array
    let pixels_xcur = u8_to_u32(to_argb(&cursor.rgba));

    // heap alloc and pass pointer to XcursorImage
    let boxed_pixels = Box::new(pixels_xcur);
    let boxed_pixels_ptr = Box::into_raw(boxed_pixels);
    let pixels = unsafe { (*boxed_pixels_ptr).as_mut_ptr() };
    // ^ must be freed!

    let nominal_size = cursor.width.max(cursor.height);

    XcursorImage {
        version: CHUNK_VERSION,
        size: nominal_size,
        delay: STATIC_DELAY,
        width: cursor.width,
        height: cursor.height,
        xhot: cursor.hotsopt_x,
        yhot: cursor.hotspot_y,
        pixels,
    }
}
