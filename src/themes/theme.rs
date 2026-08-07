//! Generic cursor theme.

use super::symlinks::get_symlinks;
use crate::{
    cursors::generic_cursor::GenericCursor, formats::inf::parse_inf_installer,
    fs_utils::resolve_icase, warn,
};

use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow, bail};
use documented::DocumentedVariants;
use fast_image_resize::ResizeAlg;
use rayon::iter::{IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator};

/// Cursor mappings stored in installer files.
#[derive(Debug, PartialEq, Eq)]
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
#[derive(Debug, PartialEq, Eq, Clone, DocumentedVariants)]
pub enum CursorType {
    // using https://github.com/khayalhus/win2xcur-batch/blob/main/map.json
    // NOTE: documentation here is displayed to users in manual installs.
    /// Description: an arrow pointing to the top-left; the default cursor
    /// Used when: nothing is happening
    /// Names: normal select, normal, pointer, arrow
    /// Looks like: ↖️ or 
    Arrow,
    /// Description: a hand pointing upwards
    /// Used when: hovering over a link or anything that you can click
    /// Names: link select, link, hand
    /// Looks like: 👆 or 
    Hand,
    /// Description: a spinning wheel or a spinner
    /// Used when: something is loading
    /// Names: busy, wait, waitcursor
    /// Looks like: 🔃 or 
    Watch,
    /// Description: a pointer with a spinning wheel or spinner
    /// Used when: something is loading in the background
    /// Names: working in background, work, appstarting
    /// Looks like: ( ↖️ and 🔃 ) or (  and  )
    LeftPtrWatch,
    /// Description: a question mark, may include a pointer
    /// Used when: hovering over something that has a tooltip
    /// Names: help select, help
    /// Looks like: [( ↖️ or  ) + ?] or ?
    Help,
    /// Description: an I-beam or a serifed I
    /// Used when: hovering over a text input field
    /// Names: text select, text, ibeam
    /// Looks like: ⌶ or 𝙸 or エ or 
    Text,
    /// Description: a pencil or pen
    /// Used when: drawing
    /// Names: handwriting, hand
    /// Looks like: ✏️ or 
    Pencil,
    /// Description: a crosshair, usually drawn as a plus
    /// Used when: taking screenshots
    /// Names: precision select, precision, crosshair, cross
    /// Looks like: + or ➕ or ✛
    Crosshair,
    /// Description: a slashed circle (a "no symbol") or crossbones
    /// Used when: indicating something can't be clicked/dragged into
    /// Names: unavailable, no
    /// Looks like: 🚫 or ☠️
    Forbidden,
    /// Description: a double-sided vertical arrow
    /// Used when: resizing something vertically
    /// Names: vertical resize, vert, sizens
    /// Looks like: ↕ or 
    NsResize,
    /// Description: a double-sided horizontal arrow
    /// Used when: resizing something horizontally
    /// Names: horizontal resize, horz, sizewe
    /// Looks like: ↔ or 
    EwResize,
    /// Description: a double-sided diagonal arrow taking the top-left and bottom-right corners
    /// Used when: resizing something from the top-left or bottom-right corner
    /// Names: diagonal resize 1, dgn1, sizenwse
    /// Looks like: ⤡ or 󰹵
    NwseResize,
    /// Description: a double-sided diagonal arrow taking the top-right and bottom-left corners
    /// Used when: resizing something from the top-right or bottom-left corner
    /// Names: diagonal resize 2, dgn2, sizenesw
    /// Looks like: ⤢ or 󰹷
    NeswResize,
    /// Description: four arrows pointing up, down, left and right joined together
    /// Used when: moving/dragging something
    /// Names: move, sizeall
    /// Looks like: ( ↔ and ↕ ) or ✥ or 󰁁
    Move,
    /// Description: an arrow facing upwards
    /// Used when: the normal cursor would be disruptive
    /// Names: alternate select, alt, uparrow
    /// Looks like: ↑ or 
    CenterPtr,
}

impl CursorType {
    pub const NUM_VARIANTS: usize = 15;
    pub const VARIANTS: [Self; Self::NUM_VARIANTS] = [
        Self::Arrow,
        Self::Hand,
        Self::Watch,
        Self::LeftPtrWatch,
        Self::Help,
        Self::Text,
        Self::Pencil,
        Self::Crosshair,
        Self::Forbidden,
        Self::NsResize,
        Self::EwResize,
        Self::NwseResize,
        Self::NeswResize,
        Self::Move,
        Self::CenterPtr,
    ];
}

/// A [`GenericCursor`] with a [`CursorType`].
#[derive(Debug)]
pub struct TypedCursor {
    inner: GenericCursor,
    /// Semantic usage of cursor, e.g for typing.
    r#type: CursorType,
}

impl TypedCursor {
    /// Creates a cursor from `mapping`.
    ///
    /// Note that this does a case-insensitive search if the path stored in `mapping` doesn't
    /// exist. This aspect is also why this isn't inside of a [`TryFrom`] implementation.
    ///
    /// ## Errors
    ///
    /// - if path contained inside of `mapping` doesn't exist, even after a case-insensitive check
    /// - generic cursor parsing fails
    pub fn from_mapping(mapping: CursorMapping) -> Result<Self> {
        let CursorMapping { path, r#type } = mapping;

        let path = if path.exists() {
            path
        } else {
            resolve_icase(&path)?.ok_or_else(|| {
                anyhow!(
                    "cursor path, path={} not found in parent (case-insensitive)",
                    path.display()
                )
            })?
        };

        let inner = GenericCursor::from_path(&path)
            .with_context(|| format!("while reading path={} as generic cursor", path.display()))?;

        Ok(Self { inner, r#type })
    }

    /// Saves as Xcursor to `dir`, along with symlinks.
    fn save_as_xcursor(&self, dir: &Path) -> Result<()> {
        let aliases = get_symlinks(&self.r#type);
        self.inner.save_as_xcursor(dir.join(aliases[0]))?;

        // relative symlink
        #[cfg(unix)]
        for symlink in &aliases[1..] {
            use std::{io, os::unix};

            match unix::fs::symlink(aliases[0], dir.join(symlink)) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(()),
                Err(e) => Err(e).with_context(|| {
                    format!(
                        "failed to create symlink {} pointing to {}",
                        dir.join(symlink).display(),
                        aliases[0]
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
    /// Validated constructor for [`CursorTheme`].
    ///
    /// ## Errors
    ///
    /// - `cursors` is empty
    /// - more cursors than variants
    /// - duplicate variants
    pub fn new(cursors: Vec<TypedCursor>, name: String) -> Result<Self> {
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
    pub fn from_installer_file(installer_file: impl AsRef<Path>) -> Result<Self> {
        let installer_file = installer_file.as_ref();
        let ext = installer_file.extension().ok_or_else(|| {
            anyhow!(
                "no extension for installer_file={}",
                installer_file.display()
            )
        })?;

        let name;
        let mappings;

        if ext.eq_ignore_ascii_case("inf") {
            (name, mappings) = parse_inf_installer(installer_file)?;
        } else if ext.eq_ignore_ascii_case("crs") {
            panic!();
        } else {
            bail!("unsupported installer file extension ext={}", ext.display());
        }

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
        let name = if self.name.is_empty() {
            warn!("blank theme name, using a placeholder name");
            String::from("placeholder_theme_name")
        } else {
            self.name.clone()
        };

        let sanitized = name.replace(['/', '\\', '.'], "_");
        let theme_dir = dir.join(sanitized);
        let cursor_dir = theme_dir.join("cursors");
        fs::create_dir_all(&cursor_dir)
            .with_context(|| format!("failed to write cursor_dir={}", cursor_dir.display()))?;

        // TODO: Replace with direct writing of tar.gz to deal with less Windows nonsense.

        // copies are not a good alternative due to storage concerns
        #[cfg(windows)]
        {
            warn!(
                "symlinks won't be created as we're on windows, a \
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
            writeln!(&mut f, "Name={}", self.name)?;
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

        for filenames in self.cursors.iter().map(|c| get_symlinks(&c.r#type)) {
            let src = filenames[0];
            let symlinks = &filenames[1..];

            for dst in symlinks {
                writeln!(&mut f, "ln -s {src} {dst}")?;
            }
        }

        Ok(())
    }
}
