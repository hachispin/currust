//! Generic cursor theme.

use super::symlinks::get_symlinks;
use crate::{
    cursors::{cursor_role::CursorRole, generic_cursor::GenericCursor},
    formats::{crs::parse_crs_installer, inf::parse_inf_installer},
    fs_utils::resolve_icase,
    warn,
};

use std::{
    fs::{self, File},
    hash::{DefaultHasher, Hash, Hasher},
    io::{self, Write},
    path::{Path, PathBuf, absolute},
};

use anyhow::{Context, Result, anyhow, bail};
use rayon::iter::{IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator};

/// Cursor mappings stored in installer files.
#[derive(Debug, PartialEq, Eq)]
pub struct CursorMapping {
    /// Semantic role of cursor.
    pub role: CursorRole,
    /// Full path to (expected) cursor.
    pub path: PathBuf,
}

/// Represents a generic cursor theme.
#[derive(Debug)]
pub struct CursorTheme {
    cursors: Vec<(CursorRole, GenericCursor)>,
    name: String,
}

impl CursorTheme {
    /// Loads a theme from cursor paths and their roles.
    ///
    /// ## Errors
    ///
    /// If the mappings are empty, contain duplicate roles, or a cursor fails to load.
    pub fn from_mappings(mappings: Vec<CursorMapping>, name: String) -> Result<Self> {
        Self::load_mappings(mappings, name, None)
    }

    fn load_mappings(
        mappings: Vec<CursorMapping>,
        name: String,
        installer_file: Option<&Path>,
    ) -> Result<Self> {
        if mappings.is_empty() {
            bail!("can't create theme with no cursors (empty)");
        }

        if mappings.len() > CursorRole::NUM_VARIANTS {
            bail!(
                "too many cursors; expected {} max for theme, got {}",
                CursorRole::NUM_VARIANTS,
                mappings.len(),
            );
        }

        let mut cursors = Vec::with_capacity(mappings.len());
        let mut fallback_hash = name.is_empty().then(DefaultHasher::new);

        for CursorMapping { path, role } in mappings {
            if cursors
                .iter()
                .any(|(existing_role, _)| existing_role == &role)
            {
                bail!("duplicate cursor type: {role:?}");
            }

            let path = resolve_icase(&path)?.ok_or_else(|| {
                anyhow!(
                    "cursor path, path={} not found in parent (case-insensitive)",
                    path.display()
                )
            })?;

            let cursor = GenericCursor::from_path(&path).with_context(|| {
                format!("while reading path={} as generic cursor", path.display())
            })?;

            if let Some(hasher) = &mut fallback_hash
                && installer_file.is_none()
            {
                role.hash(hasher);
                absolute(&path)?.hash(hasher);
            }

            cursors.push((role, cursor));
        }

        let name = if let Some(mut hasher) = fallback_hash {
            if let Some(path) = installer_file {
                absolute(path)?.hash(&mut hasher);
            }

            let name = format!("unnamed-theme-{:016x}", hasher.finish());
            warn!("blank theme name, using '{name}'");
            name
        } else {
            name
        };

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

        let (name, mappings) = if ext.eq_ignore_ascii_case("inf") {
            parse_inf_installer(installer_file)?
        } else if ext.eq_ignore_ascii_case("crs") {
            (String::new(), parse_crs_installer(installer_file)?)
        } else {
            bail!("unsupported installer file extension ext={}", ext.display())
        };

        Self::load_mappings(mappings, name, Some(installer_file))
    }

    /// Iterates over mutable cursor data in parallel.
    pub fn par_cursors_mut(&mut self) -> impl ParallelIterator<Item = &mut GenericCursor> {
        self.cursors.par_iter_mut().map(|(_, cursor)| cursor)
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
        let sanitized = self.name.replace(['/', '\\', '.'], "_");
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
            .try_for_each(|(role, cursor)| Self::save_cursor(role, cursor, &cursor_dir))?;

        /* ... write index.theme ... */
        let mut f = File::create(theme_dir.join("index.theme"))?;
        writeln!(
            &mut f,
            "# https://specifications.freedesktop.org/icon-theme/latest/#id-1.5.3.2"
        )?;
        writeln!(&mut f, "[Icon Theme]")?;

        writeln!(&mut f, "Name={}", self.name)?;

        writeln!(
            &mut f,
            "Comment=made with currust; edit index.theme to change this"
        )?;

        writeln!(&mut f, "# Inherits=fallback_theme")?;

        Ok(())
    }

    /// Saves as Xcursor to `dir`, along with symlinks.
    fn save_cursor(role: &CursorRole, cursor: &GenericCursor, dir: &Path) -> Result<()> {
        let aliases = get_symlinks(role);
        let primary = dir.join(aliases[0]);

        match fs::symlink_metadata(&primary) {
            Ok(metadata) if metadata.is_symlink() => fs::remove_file(&primary)?,
            Ok(_) => (),
            Err(e) if e.kind() == io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.into()),
        }

        cursor.save_as_xcursor(primary)?;

        // relative symlink
        #[cfg(unix)]
        for symlink in &aliases[1..] {
            use std::os::unix;

            let path = dir.join(symlink);
            match unix::fs::symlink(aliases[0], &path) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    fs::remove_file(&path)?;
                    unix::fs::symlink(aliases[0], &path)
                }
                Err(e) => Err(e),
            }
            .with_context(|| {
                format!(
                    "failed to create symlink {} pointing to {}",
                    path.display(),
                    aliases[0]
                )
            })?;
        }

        Ok(())
    }

    /// Writes a bash script to `cursor_dir` that creates symlinks for windows "compatibility".
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

        for (role, _) in &self.cursors {
            let filenames = get_symlinks(role);
            let src = filenames[0];
            let symlinks = &filenames[1..];

            for dst in symlinks {
                writeln!(&mut f, "ln -s {src} {dst}")?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::from_root;

    #[test]
    fn hashes_manual_cursor_paths_and_roles() {
        let load = |role, path: &str| {
            let mapping = CursorMapping {
                role,
                path: Path::new(from_root!("/testing/fixtures/neuro")).join(path),
            };

            CursorTheme::from_mappings(vec![mapping], String::new())
                .unwrap()
                .name
        };

        let name = load(CursorRole::Arrow, "Neuro normal.ani");
        assert!(name.starts_with("unnamed-theme-"));
        assert_eq!(name, load(CursorRole::Arrow, "Neuro normal.ani"));
        assert_ne!(name, load(CursorRole::Help, "Neuro normal.ani"));
        assert_ne!(name, load(CursorRole::Arrow, "Neuro help.ani"));
    }
}
