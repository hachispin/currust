//! Generic cursor theme.

use super::{generic_cursor::GenericCursor, symlinks::get_symlinks};

use std::{
    collections::HashMap,
    fs::{self, File},
    io::Write,
    path::Path,
};

use anyhow::{Context, Result, anyhow, bail};
use configparser::ini::Ini;
use fast_image_resize::ResizeAlg;
use rayon::iter::{IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator};

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

    fn from_inf_key(key: &str) -> Option<Self> {
        Some(match key {
            "pointer" => Self::Arrow,
            "help" => Self::Help,
            "work" | "working" => Self::LeftPtrWatch,
            "busy" => Self::Watch,
            "cross" | "precision" => Self::Crosshair,
            "text" => Self::Text,
            "hand" => Self::Pencil,
            "unavailable" | "unavailiable" => Self::Forbidden,
            "vert" => Self::NsResize,
            "horz" => Self::EwResize,
            "dgn1" => Self::NwseResize,
            "dgn2" => Self::NeswResize,
            "move" => Self::Move,
            "alternate" => Self::CenterPtr,
            "link" => Self::Hand,
            _ => {
                return None;
            }
        })
    }
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

    fn save_as_xcursor(&self, dir: &Path) -> Result<()> {
        if !dir.is_dir() {
            bail!("path={} must be dir", dir.display());
        }

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

/// Helper function for filtering.
///
/// This trims quotes, since [`configparser`] takes _everything_ as a string.
///
/// For example: `key = "value"` means `config["key"] == "\"value\""`.
fn dequote_value(entry: (&String, &Option<String>)) -> Option<(String, String)> {
    match entry {
        (k, Some(v)) => Some((
            k.clone(),
            v.strip_suffix('"')
                .unwrap_or_default()
                .strip_prefix('"')
                .unwrap_or_default()
                .to_string(),
        )),
        (k, None) => {
            // side effect but shhh
            eprintln!("[warning] key={k} has value None");
            None
        }
    }
}

/// Represents a generic cursor theme.
#[derive(Debug)]
pub struct CursorTheme {
    cursors: Vec<TypedCursor>,
    name: String,
}

impl CursorTheme {
    fn new(cursors: Vec<TypedCursor>, name: &str) -> Result<Self> {
        let name = name.to_string();
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

    /// Reads provided cursors as a path using `inf_path` for mappings.
    ///
    /// ## Errors
    ///
    /// Mostly from parsing the INF file and filesystem operations.
    pub fn from_theme_dir<P: AsRef<Path>>(theme_dir: P) -> Result<Self> {
        let theme_dir = theme_dir.as_ref();
        let theme_dir_display = theme_dir.display();

        if !theme_dir.is_dir() {
            bail!("theme_dir={theme_dir_display} must be a dir");
        }

        let inf = Self::extract_installer(&theme_dir)?;

        // strings section has key-value pairs like:
        // cursor_type = path_to_cursor
        // e.g, pointer = "01-Normal.ani"
        let mappings: HashMap<_, _> = inf
            .get("strings")
            .ok_or_else(|| {
                anyhow!("no 'strings' section found in inf file in theme_dir={theme_dir_display}")
            })?
            .iter()
            .filter_map(dequote_value)
            .collect();

        // could cause conflicts if there are
        // multiple unnamed themes, should fix
        let name = mappings
            .get("scheme_name")
            .map_or_else(|| "unnamed theme", String::as_str);

        let mut typed_cursors = Vec::with_capacity(mappings.len());
        for (key, cursor_path) in &mappings {
            // info that's not related to cursor mappings
            const SKIP_KEYS: [&str; 2] = ["cur_dir", "scheme_name"];
            if SKIP_KEYS.contains(&key.as_str()) {
                continue;
            }

            let Some(r#type) = CursorType::from_inf_key(key) else {
                // these keys are expected but are intentionally
                // skipped as they have no xcursor equivalent
                if key != "pin" && key != "person" {
                    eprintln!("[warning] unknown key={key}, skipping");
                }

                continue;
            };

            let cursor_path = theme_dir.join(cursor_path);

            // usually occurs because windows has case-insensitive paths
            // e.g, precision == Precision and what-not
            let cursor_path = if cursor_path.exists() {
                cursor_path
            } else {
                let cursor_path_cmp = cursor_path.as_os_str().to_ascii_lowercase();
                let parent = &cursor_path
                    .parent()
                    .ok_or_else(|| anyhow!("no parent in cursor path for key={key}"))?;

                parent
                    .read_dir()?
                    .filter_map(Result::ok)
                    .map(|e| e.path())
                    .find(|p| p.as_os_str().to_ascii_lowercase() == cursor_path_cmp)
                    .ok_or_else(|| {
                        anyhow!(
                            "can't find cursor_path={} in its parent={}",
                            cursor_path.display(),
                            parent.display()
                        )
                    })?
            };

            let cursor = GenericCursor::from_path(cursor_path)?;
            let typed_cursor = TypedCursor::new(cursor, r#type);
            typed_cursors.push(typed_cursor);
        }

        Self::new(typed_cursors, name)
    }

    /// Returns the theme installer file (INF) in `dir`.
    ///
    /// This does not search recursively.
    ///
    /// ## Errors
    ///
    /// If `dir` is... not a dir, or if there are:
    ///
    /// - multiple valid candidates
    /// - no candidates
    /// - reading failures
    fn extract_installer(dir: &Path) -> Result<HashMap<String, HashMap<String, Option<String>>>> {
        let dir_display = dir.display();

        if !dir.is_dir() {
            bail!("expected path={dir_display} to be dir");
        }

        let inf_paths: Vec<_> = dir
            .read_dir()?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("inf"))
            })
            .collect();

        let infs: Vec<_> = inf_paths
            .into_iter()
            .map(|p| {
                let inf_string = fs::read_to_string(&p)?;
                let inf = Ini::new()
                    .read(inf_string)
                    .map_err(|e| anyhow!("failed to read ini from {}: {e}", p.display()))?;

                Ok(inf)
            })
            .collect::<Result<_>>()?;

        let installers: Vec<_> = infs
            .into_iter()
            .filter(|inf| {
                inf.get("version").is_some_and(|kv| {
                    kv.get("signature")
                        .is_some_and(|v| *v == Some("\"$CHICAGO$\"".to_string()))
                })
            })
            .collect();

        if installers.len() > 1 {
            bail!("found more than one viable installer INF file in dir={dir_display}");
        }

        if let Some(ins) = installers.first().cloned() {
            Ok(ins)
        } else {
            bail!("no viable installer INF file found in dir={dir_display}");
        }
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
        fs::create_dir_all(&cursor_dir)?;

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
        writeln!(&mut f, "Name={}", &self.name)?;
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

        if !cursor_dir.is_dir() {
            bail!("dir={dir_display} is not a dir")
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
