//! Utilities related to paths.

use anyhow::{Result, bail};

use std::{
    borrow::ToOwned,
    path::{Path, PathBuf},
};

use crate::warn;

/// Resolves each component in `path` case-insensitively.
///
/// Mostly used for Windows to Linux path conversions.
///
/// ## Errors
///
/// If multiple candidates are found, or for general fs issues.
pub fn resolve_icase(path: &Path) -> Result<Option<PathBuf>> {
    let path_display = path.display();

    // windows should just always hit this, methinks
    if path.try_exists()? {
        return Ok(Some(path.to_path_buf()));
    }

    let mut resolved = PathBuf::new();

    for component in path.components() {
        if resolved.join(component).try_exists()? {
            resolved.push(component);
            continue;
        }

        let component = component.as_os_str();

        let found: Vec<_> = read_dir(&resolved, true, true)?
            .filter_map(|p| p.file_name().map(ToOwned::to_owned))
            .filter(|name| name.eq_ignore_ascii_case(component))
            .collect();

        match found.as_slice() {
            [] => return Ok(None),
            [name] => resolved.push(name),
            _ => bail!(
                "multiple candidates found for case-insensitive \
                lookup in parent={} for path={path_display}",
                resolved.display()
            ),
        }
    }

    Ok(Some(resolved))
}

/// Attempts to find files in `dir` with file extensions in `extensions`.
///
/// This assumes that `dir` exists and does not search recursively for the given extensions.
/// The extensions must _not_ be prefixed with a dot (e.g., "png" instead of ".png").
///
/// ## Errors
///
/// - if `dir` is not a directory
/// - if [`Path::read_dir`] fails
pub fn find_extensions_icase(
    dir: &Path,
    extensions: &[&str],
) -> Result<impl Iterator<Item = PathBuf>> {
    let dir_display = dir.display();
    if !dir.metadata()?.is_dir() {
        bail!("expected dir={dir_display} to be a directory");
    }

    Ok(read_dir(dir, true, false)?.filter(|p| {
        p.extension()
            .is_some_and(|ext| extensions.iter().any(|ele| ext.eq_ignore_ascii_case(ele)))
    }))
}

/// Helper function for reading `dir` robustly.
fn read_dir(
    dir: &Path,
    allow_file: bool,
    allow_dir: bool,
) -> Result<impl Iterator<Item = PathBuf>> {
    if !allow_file && !allow_dir {
        bail!("both file and dir not allowed - most likely unintended");
    }

    Ok(dir
        .read_dir()?
        .filter_map(|e| {
            e.inspect_err(|err| {
                warn!("couldn't read entry in dir={}: {err}", dir.display());
            })
            .ok()
        })
        .map(|e| e.path())
        .filter(move |p| {
            p.metadata() // follows symlinks
                .inspect_err(|err| {
                    warn!("failed to read metadata of path, p={}: {err}", p.display());
                })
                .is_ok_and(|m| allow_file && m.is_file() || allow_dir && m.is_dir())
        }))
}
