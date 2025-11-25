//! WIP

//  https://man.archlinux.org/man/Xcursor.3

/// Converts the bytes in `rgba` to ARGB format in-place.
pub fn to_argb(rgba: &mut Vec<u8>) {
    assert!(
        rgba.len() % 4 == 0,
        "invalid RGBA, each pixel should have 4 channels"
    );

    for pixel in rgba.chunks_exact_mut(4) {
        pixel.swap(0, 3); // AGBR
        pixel.swap(1, 2); // ABGR
        pixel.swap(1, 3); // ARGB
    }
}
