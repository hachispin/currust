//! Parses CRS files from RW Cursor editor.
//!
//! Pretty sure CRS is just TOML with required sections.

use super::CursorMapping;
use crate::themes::theme::CursorType;

use std::{fs, path::Path};

use anyhow::{Result, anyhow, bail};
use configparser::ini::Ini;

/// Section names in CRS files.
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

/// Attempts to extract mappings out of a CRS file.
///
/// ## Errors
///
/// If file is failed to be read or has unexpected sections.
/// Note that missing sections are not treated as an error.
pub fn parse_crs_installer(crs_path: &Path, theme_dir: &Path) -> Result<Vec<CursorMapping>> {
    let crs_string = fs::read_to_string(crs_path)?;
    let crs = Ini::new()
        .read(crs_string)
        .map_err(|e| anyhow!("failed to read crs, error e={e}"))?;

    let mut mappings = Vec::with_capacity(CursorType::NUM_VARIANTS);

    for section_name in crs.keys() {
        let Some(r#type) = section_to_type(section_name) else {
            bail!("unexpected section in crs file (please report), section={section_name}");
        };

        let relative_path = crs.get(section_name).and_then(|s| s.get("path"));

        let Some(Some(relative_path)) = relative_path else {
            eprintln!("[warning] skipping section_name={section_name}");
            continue;
        };

        let path = theme_dir.join(relative_path);
        mappings.push((path, r#type));
    }

    Ok(mappings)
}
