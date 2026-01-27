//! Generic cursor theme.

use super::generic_cursor::GenericCursor;

use std::{
    fs::{self},
    os::unix,
    path::Path,
};

use anyhow::{Result, anyhow, bail};
use configparser::ini::Ini;

/// Represents the possible cursors that exist in both Windows and Linux (X11).
///
/// Some cursors, such as [`XcursorType::Crosshair`], have symlinks to
/// Xcursors that aren't _exactly_ the same, such as `color-picker`.
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
            "work" => Self::LeftPtrWatch,
            "busy" => Self::Watch,
            "cross" => Self::Crosshair,
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
                eprintln!("unexpected INF key={key}");
                return None;
            }
        })
    }
}

/// A [`GenericCursor`] with a [`CursorType`].
#[derive(Debug)]
pub struct TypedCursor {
    inner: GenericCursor,
    r#type: CursorType,
    symlinks: &'static [&'static str],
}

impl TypedCursor {
    fn new(xcursor: GenericCursor, r#type: CursorType) -> Self {
        let symlinks = symlinks::get_symlinks(&r#type);

        Self {
            inner: xcursor,
            r#type,
            symlinks,
        }
    }

    fn save_as_xcursor(&self, dir: &Path) -> Result<()> {
        if !dir.is_dir() {
            bail!("path={} must be dir", dir.display());
        }

        self.inner.save_as_xcursor(dir.join(self.symlinks[0]))?;

        // relative symlink
        for symlink in &self.symlinks[1..] {
            #[cfg(not(target_os = "windows"))]
            unix::fs::symlink(self.symlinks[0], dir.join(symlink))?;
        }

        Ok(())
    }
}

/// Represents a generic cursor theme.
#[derive(Debug)]
pub struct CursorTheme {
    cursors: Vec<TypedCursor>,
}

impl CursorTheme {
    fn new(cursors: Vec<TypedCursor>) -> Result<Self> {
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

        Ok(Self { cursors })
    }

    /// Reads provided cursors as a path using `inf_path` for mappings.
    ///
    /// ## Errors
    ///
    /// Mostly from parsing the INF file and filesystem operations.
    pub fn from_theme_dir(theme_dir: &Path, inf_path: &Path) -> Result<Self> {
        if !theme_dir.is_dir() {
            bail!("theme_dir={} must be a dir", theme_dir.display());
        }

        let raw_ini = fs::read_to_string(inf_path)?;
        let ini = Ini::new()
            .read(raw_ini)
            .map_err(|e| anyhow!("couldn't parse inf_path={}: {e}", inf_path.display()))?;

        // strings section has key-value pairs like:
        // cursor_type = path_to_cursor
        // e.g, pointer = "01-Normal.ani"
        let mappings = &ini
            .get("strings")
            .ok_or_else(|| anyhow!("no 'strings' section found in ini"))?;

        let mut typed_cursors = Vec::with_capacity(mappings.len());
        for (key, cursor_path) in *mappings {
            let Some(r#type) = CursorType::from_inf_key(key) else {
                continue;
            };

            let Some(cursor_path) = cursor_path else {
                bail!("no path found for key={key}");
            };

            let cursor_path = theme_dir.join(cursor_path);
            let Some(ext) = cursor_path.extension() else {
                bail!("no extension")
            };

            let is_animated = ext == "ani";
            let cursor = if is_animated {
                GenericCursor::from_ani_path(cursor_path)
            } else {
                GenericCursor::from_cur_path(cursor_path)
            }?;

            let typed_cursor = TypedCursor::new(cursor, r#type);
            typed_cursors.push(typed_cursor);
        }

        Self::new(typed_cursors)
    }

    /// Saves current theme.
    ///
    /// This creates symlinks unless the target OS is Windows,
    /// in which case, a warning is logged and we continue.
    ///
    /// ## Errors
    ///
    /// If writing Xcursor/symlinks fail.
    pub fn save_as_xcursors(&self, dir: &Path) -> Result<()> {
        // could create copies instead but that doesn't scale well...
        // xcursor themes can already be fat (uncompressed bitmaps...)
        // multiply by symlinks and -- 💥 boom. hundreds of megabytes...
        //
        // for reference, breeze dark theme on fedora kde is 15MB (!)
        #[cfg(target_os = "windows")]
        eprintln!("[warning] symlinks won't be created as we're on windows");

        for cursor in &self.cursors {
            cursor.save_as_xcursor(dir)?;
        }

        Ok(())
    }
}

/// Symlinks for X11 cursor names in [`CursorTheme`].
///
/// The first string in each list is treated as
/// the "concrete" file that symlinks point to.
///
/// Courtesy of [win2xcur-batch](https://github.com/khayalhus/win2xcur-batch/blob/main/map.json).
mod symlinks {
    use super::CursorType;

    pub const ARROW: &[&str] = &["arrow", "default", "left_ptr", "top_left_arrow"];
    pub const HAND: &[&str] = &[
        "alias",
        "dnd-link",
        "hand",
        "hand1",
        "hand2",
        "link",
        "openhand",
        "pointer",
        "pointing_hand",
        "3085a0e285430894940527032f8b26df",
        "640fb0e74195791501fd1ed57b41487f",
        "9d800788f1b08800ae810202380a0822",
        "a2a266d0498c3104214a47bd64ab0fc8",
        "b66166c04f8c3109214a4fbd64a50fc8",
        "e29285e634086352946a0e7090d73106",
    ];

    pub const WATCH: &[&str] = &["wait", "watch"];
    pub const LEFT_PTR_WATCH: &[&str] = &[
        "half-busy",
        "left_ptr_watch",
        "progress",
        "00000000000000020006000e7e9ffc3f",
        "08e8e1c95fe2fc01f976f1e063a24ccd",
        "3ecb610c1bf2410f44200f48c40d3599",
    ];

    pub const HELP: &[&str] = &[
        "dnd-ask",
        "help",
        "left_ptr_help",
        "question_arrow",
        "whats_this",
        "5c6cd98b3f3ebcb1f9c7f1c204630408",
        "d9ce0ab605698f320427677b458ad60b",
    ];

    pub const TEXT: &[&str] = &["ibeam", "text", "xterm", "vertical-text"];
    pub const PENCIL: &[&str] = &["draft", "pencil"];
    pub const CROSSHAIR: &[&str] = &[
        "cell",
        "color-picker",
        "cross_reverse",
        "cross",
        "crosshair",
        "diamond_cross",
        "plus",
        // "size_all", -- better as move
        "tcross",
    ];

    pub const FORBIDDEN: &[&str] = &[
        "circle",
        "crossed_circle",
        "dnd-no-drop",
        "forbidden",
        "not-allowed",
        "no-drop",
        "pirate",
        "03b6e0fcb3499374a867c041f52298f0",
    ];

    pub const NS_RESIZE: &[&str] = &[
        "top_side",
        "bottom_side",
        "n-resize",
        "ns-resize",
        "row-resize",
        "s-resize",
        "sb_v_double_arrow",
        "size_ver",
        "split_v",
        "v_double_arrow",
        "00008160000006810000408080010102",
        "2870a09082c103050810ffdffffe0204",
    ];

    pub const EW_RESIZE: &[&str] = &[
        "col-resize",
        "down-arrow",
        "e-resize",
        "ew-resize",
        "h_double_arrow",
        "left_side",
        "left-arrow",
        "right_side",
        "right-arrow",
        "sb_h_double_arrow",
        "size_hor",
        "split_h",
        "w-resize",
        "14fef782d02440884392942c11205230",
        "028006030e0e7ebffc7f7070c0600140",
    ];

    pub const NWSE_RESIZE: &[&str] = &[
        "bottom_right_corner",
        "nw-resize",
        "nwse-resize",
        "se-resize",
        "size_fdiag",
        "top_left_corner",
        "ul_angle",
        "c7088f0f3e6c8088236ef8e1e3e70000",
    ];

    pub const NESW_RESIZE: &[&str] = &[
        "bd_double_arrow",
        "bottom_left_corner",
        "fd_double_arrow",
        "ne-resize",
        "nesw-resize",
        "size_bdiag",
        "sw-resize",
        "top_right_corner",
        "ur_angle",
        "fcf1c3c7cd4491d801f1e1c78f100000",
    ];

    pub const MOVE: &[&str] = &[
        "size_all",
        "all-scroll",
        "closedhand",
        "dnd-move",
        "dnd-none",
        "fleur",
        "grab",
        "grabbing",
        "move",
        "4498f0e0c1937ffe01fd06f973665830",
        "9081237383d90e509aa00f00170e968f",
    ];

    pub const CENTER_PTR: &[&str] = &[
        // "top_side", -- better mapped to ns-resize
        "up_arrow",
        "right_ptr",
        "draft_large",
        "draft_small",
        "up-arrow",
        "center_ptr",
    ];

    pub fn get_symlinks(r#type: &CursorType) -> &'static [&'static str] {
        match r#type {
            CursorType::Arrow => ARROW,
            CursorType::Hand => HAND,
            CursorType::Watch => WATCH,
            CursorType::LeftPtrWatch => LEFT_PTR_WATCH,
            CursorType::Help => HELP,
            CursorType::Text => TEXT,
            CursorType::Pencil => PENCIL,
            CursorType::Crosshair => CROSSHAIR,
            CursorType::Forbidden => FORBIDDEN,
            CursorType::NsResize => NS_RESIZE,
            CursorType::EwResize => EW_RESIZE,
            CursorType::NwseResize => NWSE_RESIZE,
            CursorType::NeswResize => NESW_RESIZE,
            CursorType::Move => MOVE,
            CursorType::CenterPtr => CENTER_PTR,
        }
    }
}
