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
/// - `file_path` is not a file
/// - multiple files match `file_path`
pub fn find_icase(file_path: &Path) -> Result<Option<PathBuf>> {
    let file_path_display = file_path.display();

    if file_path.try_exists()? {
        if file_path.metadata()?.is_file() {
            return Ok(Some(file_path.to_path_buf()));
        }

        bail!("file_path={file_path_display} exists but is not a file");
    }

    let parent = file_path.parent().ok_or_else(|| {
        anyhow!("no parent found for file_path={file_path_display} during case-insensitive lookup")
    })?;
    let parent_display = parent.display();

    let filename = file_path
        .file_name()
        .ok_or_else(|| anyhow!("no filename for file_path={file_path_display}"))?;

    let found: Vec<_> = read_dir_files(parent)?
        .filter(|p| {
            p.file_name()
                .is_some_and(|name| name.eq_ignore_ascii_case(filename))
        })
        .collect();

    if found.len() > 1 {
        bail!(
            "multiple candidates found for case-insensitive lookup \
            in parent={parent_display} for filename={file_path_display}"
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
pub fn find_extensions_icase(
    dir: &Path,
    extensions: &[&str],
) -> Result<impl Iterator<Item = PathBuf>> {
    let dir_display = dir.display();
    if !dir.metadata()?.is_dir() {
        bail!("expected dir={dir_display} to be a directory");
    }

    Ok(read_dir_files(dir)?.filter(|p| {
        p.extension()
            .is_some_and(|ext| extensions.iter().any(|ele| ext.eq_ignore_ascii_case(ele)))
    }))
}

/// Helper function for reading `dir` robustly.
///
/// The returned iterator only yields files.
fn read_dir_files(dir: &Path) -> Result<impl Iterator<Item = PathBuf>> {
    Ok(dir
        .read_dir()?
        .filter_map(|e| {
            e.inspect_err(|err| {
                eprintln!(
                    "[warning] couldn't read entry in dir={}: {err}",
                    dir.display()
                );
            })
            .ok()
        })
        .map(|e| e.path())
        .filter(|p| {
            p.metadata()
                .inspect_err(|err| {
                    eprintln!(
                        "[warning] failed to read metadata of path, p={}: {err}",
                        p.display()
                    );
                })
                .ok()
                .is_some_and(|m| m.is_file())
        }))
}
