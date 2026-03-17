//! Generic cursor theme.

use super::symlinks::get_symlinks;
use crate::{
    cursors::generic_cursor::GenericCursor,
    formats::{crs::parse_crs_installer, inf::parse_inf_installer},
    fs_utils::{find_extensions_icase, find_icase},
};

use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow, bail};
use fast_image_resize::ResizeAlg;
use rayon::iter::{IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator};

/// Cursor mappings stored in installer files.
#[derive(Debug, PartialEq)]
pub struct CursorMapping {
    /// Semantic role of cursor.
    pub r#type: CursorType,
    /// Full path to (expected) cursor.
    pub path: PathBuf,
}

/// Represents the possible cursors that exist in both Windows and Linux (X11).
///
/// Some cursors, such as `Crosshair`, have symlinks to Xcursors
/// that aren't _exactly_ the same, such as `color-picker`.
#[derive(Debug, PartialEq, Clone)]
pub enum CursorType {
    // using https://github.com/khayalhus/win2xcur-batch/blob/main/map.json
    /// The default, left pointer.
    Arrow,
    /// Displayed when hovering over a link, usually a hand ( 👆 ).
    Hand,
    /// Displayed when something's loading, usually a spinning wheel ( 🔃 ).
    Watch,
    /// Similar to [`CursorType::Watch`], but with the loading
    /// wheel to the side of [`CursorType::Arrow`], usually.
    LeftPtrWatch,
    /// Usually a question mark. ( ?/❔/❓ )
    Help,
    /// Displayed when hovering over a text field, usually looks like an "I".
    Text,
    /// Displayed when drawing, usually a pencil. ( ✏️ )
    Pencil,
    /// Usually a "plus symbol". ( +/➕/✛ )
    Crosshair,
    /// Usually a "no symbol". ( 🚫 )
    Forbidden,
    /// Displayed when scaling vertically, usually
    /// a bi-directional, vertical arrow. ( ↕ )
    NsResize,
    /// Displayed when scaling horizontally, usually
    /// a bi-directional, horizontal arrow. ( ↔ )
    EwResize,
    /// Displayed when scaling from the bottom-right/top-left
    /// corner, usually a bi-directional, diagonal arrow. ( ⤡ )
    NwseResize,
    /// Displayed when scaling from the top-right/bottom-left corner,
    /// usually a bi-directional, diagonal arrow. ( ⤢ )
    NeswResize,
    /// Displayed when moving something, usually two bi-directional
    /// vertical and horizontal arrows, stacked on top of each other.
    Move,
    /// Usually a centered pointer. ( ↑ )
    ///
    /// This has a lot of symlinks to some cursors that aren't really
    /// closely related, since this is mapping "alternate" from Windows.
    CenterPtr,
}

impl CursorType {
    const NUM_VARIANTS: usize = 15;
}

/// A [`GenericCursor`] with a [`CursorType`].
#[derive(Debug)]
pub struct TypedCursor {
    inner: GenericCursor,
    /// Semantic usage of cursor, e.g for typing.
    r#type: CursorType,
    /// First entry is the filename, rest are used as symlinks.
    aliases: &'static [&'static str],
}

impl TypedCursor {
    fn new(xcursor: GenericCursor, r#type: CursorType) -> Self {
        let aliases = get_symlinks(&r#type);

        Self {
            inner: xcursor,
            r#type,
            aliases,
        }
    }

    /// Creates a cursor from `mapping`.
    ///
    /// ## Errors
    ///
    /// - if path contained inside of `mapping` doesn't exist,
    ///   even after a case-insensitive check
    /// - generic cursor parsing fails
    fn from_mapping(mapping: CursorMapping) -> Result<Self> {
        let path = mapping.path;
        let path = if path.exists() {
            path
        } else {
            find_icase(&path)?.ok_or_else(|| {
                anyhow!(
                    "cursor path, path={} not found in parent (case-insensitive)",
                    path.display()
                )
            })?
        };

        Ok(Self::new(
            GenericCursor::from_path(&path).with_context(|| {
                format!("while reading path={} as generic cursor", path.display())
            })?,
            mapping.r#type,
        ))
    }

    /// Saves as Xcursor to `dir`, along with symlinks.
    fn save_as_xcursor(&self, dir: &Path) -> Result<()> {
        self.inner.save_as_xcursor(dir.join(self.aliases[0]))?;

        // relative symlink
        #[cfg(unix)]
        for symlink in &self.aliases[1..] {
            use std::{io, os::unix};

            match unix::fs::symlink(self.aliases[0], dir.join(symlink)) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(()),
                Err(e) => Err(e).with_context(|| {
                    format!(
                        "failed to create symlink {} pointing to {}",
                        dir.join(symlink).display(),
                        self.aliases[0]
                    )
                }),
            }?;
        }

        Ok(())
    }
}

/// Represents a generic cursor theme.
#[derive(Debug)]
pub struct CursorTheme {
    cursors: Vec<TypedCursor>,
    name: String,
}

impl CursorTheme {
    fn new(cursors: Vec<TypedCursor>, name: String) -> Result<Self> {
        if cursors.is_empty() {
            bail!("can't create theme with no cursors (empty)");
        }

        if cursors.len() > CursorType::NUM_VARIANTS {
            bail!(
                "too many cursors; expected {} max for theme, got {}",
                CursorType::NUM_VARIANTS,
                cursors.len(),
            );
        }

        let mut seen = Vec::new();
        for cursor in &cursors {
            if seen.contains(&cursor.r#type) {
                bail!("duplicate cursor type: {:?}", cursor.r#type);
            }

            seen.push(cursor.r#type.clone());
        }

        Ok(Self { cursors, name })
    }

    /// Reads provided cursors as a path.
    ///
    /// ## Errors
    ///
    /// Mostly from parsing the INF file and filesystem operations.
    pub fn from_theme_dir<P: AsRef<Path>>(theme_dir: P) -> Result<Self> {
        let theme_dir = theme_dir.as_ref();
        let installers: Vec<_> = find_extensions_icase(theme_dir, &["inf", "crs"])?.collect();

        if installers.len() > 1 {
            bail!("found more than one installer (INF/CRS) file");
        }

        let Some(installer) = installers.first().cloned() else {
            bail!("no installer (INF/CRS) file found");
        };

        // a bit ugly
        let (name, mappings) = if installer
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("inf"))
        {
            parse_inf_installer(&installer, theme_dir)
                .with_context(|| format!("while attempting to parse {}", installer.display()))?
        } else {
            (String::new(), parse_crs_installer(&installer, theme_dir)?)
        };

        let typed_cursors: Vec<_> = mappings
            .into_iter()
            .map(TypedCursor::from_mapping)
            .collect::<Result<_>>()?;

        Self::new(typed_cursors, name)
    }

    /// Adds scale to all cursors for the current theme.
    ///
    /// ## Errors
    ///
    /// From [`GenericCursor::add_scale`].
    pub fn add_scale(&mut self, scale_factor: f64, algorithm: ResizeAlg) -> Result<()> {
        self.cursors
            .par_iter_mut()
            .try_for_each(|c| c.inner.add_scale(scale_factor, algorithm))?;

        Ok(())
    }

    /// Saves current theme in `dir`, which is created if it doesn't already exist.
    ///
    /// This creates symlinks unless the target OS is Windows,
    /// in which case, a warning is logged and we continue.
    ///
    /// ## Errors
    ///
    /// If writing Xcursor/symlinks fail.
    pub fn save_as_x11_theme(&self, dir: &Path) -> Result<()> {
        let theme_dir = dir.join(&self.name);
        let cursor_dir = theme_dir.join("cursors");
        fs::create_dir_all(&cursor_dir)
            .with_context(|| format!("failed to write cursor_dir={}", cursor_dir.display()))?;

        // copies are *not* a good alternative here.
        // xcursor can get very large, very quickly
        // and there are wayy too many symlinks.
        #[cfg(windows)]
        {
            eprintln!(
                "[warning] symlinks won't be created as we're on windows, a \
                bash script for usage on linux will be created instead"
            );

            self.write_symlink_script(&cursor_dir)?;
        }

        self.cursors
            .par_iter()
            .try_for_each(|c| c.save_as_xcursor(&cursor_dir))?;

        /* ... write index.theme ... */
        let mut f = File::create(theme_dir.join("index.theme"))?;
        writeln!(
            &mut f,
            "# https://specifications.freedesktop.org/icon-theme/latest/#id-1.5.3.2"
        )?;
        writeln!(&mut f, "[Icon Theme]")?;

        // should probably use option but i'm lazy
        if self.name.is_empty() {
            writeln!(&mut f, "# Name=theme_name")?;
        } else {
            writeln!(&mut f, "Name={}", &self.name)?;
        }

        writeln!(
            &mut f,
            "Comment=made with currust; edit index.theme to change this"
        )?;

        writeln!(&mut f, "# Inherits=fallback_theme")?;

        Ok(())
    }

    /// Writes a bash script to `cursor_dir` that
    /// creates symlinks for windows "compatibility".
    ///
    /// This expects the Xcursor files (src) to already be written.
    #[cfg(windows)]
    fn write_symlink_script(&self, cursor_dir: &Path) -> Result<()> {
        let dir_display = cursor_dir.display();

        if !cursor_dir.exists() {
            bail!("dir={dir_display} doesn't exist");
        }

        // unfortunately can't set chmod +x permission here
        let mut f = File::create(cursor_dir.join("write_symlinks.sh"))?;
        writeln!(&mut f, "#!/usr/bin/env bash\n")?;

        for filenames in self.cursors.iter().map(|c| c.aliases) {
            let src = filenames[0];
            let symlinks = &filenames[1..];

            for dst in symlinks {
                writeln!(&mut f, "ln -s {src} {dst}")?;
            }
        }

        Ok(())
    }
}
