//! Module pertaining to the Windows CUR format, which
//! is quite similar to the [ICO](https://en.wikipedia.org/wiki/ICO_(file_format)) format.

use super::common::CursorImage;

use std::{fs::File, path::Path};

use anyhow::{Context, Result};
use ico::IconDir;

/// Reads and parses a cursor from `cur_path`.
///
/// ## Errors
///
/// If a file handle to `cur_path` can't be opened,
/// or the file stored is not a CUR file.
pub fn read_cur<P: AsRef<Path>>(cur_path: P) -> Result<Vec<CursorImage>> {
    let cur_path = cur_path.as_ref();
    let cur_path_display = cur_path.display();

    let handle = File::open(cur_path)
        .with_context(|| format!("failed to read from cur_path={cur_path_display}"))?;

    let icon_dir = IconDir::read(handle)
        .with_context(|| format!("failed to read `IconDir` from cur_path={cur_path_display}"))?;

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

    Ok(images)
}
