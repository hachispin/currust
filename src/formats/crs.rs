//! Parses CRS files from RW Cursor editor.
//!
//! CRS is just TOML with required sections.

use crate::themes::theme::{CursorMapping, CursorType};

use std::{fs, path::Path};

use anyhow::{Result, anyhow};
use configparser::ini::Ini;

/// Section names in CRS files.
///
/// NOTE: may not be all possible sections!
fn section_to_type(section: &str) -> Option<CursorType> {
    use CursorType::*;

    Some(match section {
        "arrow" | "default" => Arrow, // unsure
        "help" => Help,
        "appstarting" => LeftPtrWatch,
        "wait" => Watch,
        "crosshair" => Crosshair,
        "ibeam" => Text,
        "nwpen" => Pencil,
        "no" => Forbidden,
        "sizenesw" => NeswResize,
        "sizens" => NsResize,
        "sizewe" => EwResize,
        "sizenwse" => NwseResize,
        "sizeall" => Move,
        "uparrow" => CenterPtr,
        "hand" => Hand,
        _ => {
            return None;
        }
    })
}

#[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
pub fn parse_crs_installer(crs_path: &Path, theme_dir: &Path) -> Result<Vec<CursorMapping>> {
    let crs_string = fs::read_to_string(crs_path)?;
    let crs = Ini::new()
        .read(crs_string)
        .map_err(|e| anyhow!("failed to read crs, error e={e}"))?;

    let mut mappings = Vec::with_capacity(16);

    // TODO: refactor this, it looks a little repulsive
    for section_name in crs.keys() {
        // some of these should probably be bails
        let Some(r#type) = section_to_type(section_name) else {
            eprintln!("[warning] skipping unexpected section in crs file, section={section_name}");
            continue;
        };

        let Some(section) = crs.get(section_name) else {
            eprintln!("[warning] skipping section_name={section_name}");
            continue;
        };

        let Some(path_value) = section.get("path") else {
            eprintln!("[warning] skipping section_name={section_name}");
            continue;
        };

        let Some(path_value) = path_value else {
            eprintln!("[warning] no value for section_name={section_name}");
            continue;
        };

        let path = theme_dir.join(path_value);
        mappings.push(CursorMapping { r#type, path });
    }

    Ok(mappings)
}
