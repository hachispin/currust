//! Utilities related to paths.

use anyhow::{Result, anyhow, bail};
use std::path::{Path, PathBuf};

/// Attempts to find `file_path` by searching through it's parent dir.
///
/// This does not search recursively. Also, `file_path.file_name()`
/// not being found in its parent dir isn't considered an error.
///
/// ## Errors
///
/// - `file_path` doesn't have a parent
/// - multiple files match `file_path`
pub fn find_icase(file_path: &Path) -> Result<Option<PathBuf>> {
    let file_path_display = file_path.display();

    if !file_path.is_file() {
        bail!("expected file={file_path_display} to be a file");
    }

    if file_path.exists() {
        return Ok(Some(file_path.to_path_buf()));
    }

    let parent = file_path.parent().ok_or_else(|| {
        anyhow!("no parent found for file={file_path_display} during case-insensitive lookup")
    })?;

    let parent_display = parent.display();
    let filename = file_path
        .file_name()
        .ok_or_else(|| anyhow!("no filename for file_path={file_path_display}"))?;

    let filename_cmp = filename.to_ascii_lowercase();

    let found: Vec<_> = parent
        .read_dir()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.as_os_str().to_ascii_lowercase() == filename_cmp)
        .collect();

    if found.len() > 1 {
        bail!(
            "multiple candidates found for case-insensitive lookup in \
            parent={parent_display} for filename={file_path_display}"
        );
    }

    Ok(found.first().cloned())
}

/// Attempts to find files in `dir` with file extensions in `extensions`.
///
/// This is case-insensitive and not recursive.
///
/// ## Errors
///
/// - if `dir` is not a directory
/// - if [`Path::read_dir`] fails
pub fn find_extensions_icase(dir: &Path, extensions: &[&str]) -> Result<Vec<PathBuf>> {
    let dir_display = dir.display();

    if !dir.is_dir() {
        bail!("expected dir={dir_display} to be a directory");
    }

    let found: Vec<_> = dir
        .read_dir()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|ext| extensions.iter().any(|ele| ext.eq_ignore_ascii_case(ele)))
        })
        .collect();

    Ok(found)
}
