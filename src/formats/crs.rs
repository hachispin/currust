//! Parses CRS files from RW Cursor editor.
//!
//! Pretty sure CRS is just TOML with required sections.

use crate::{
    themes::theme::{CursorMapping, CursorType},
    warn,
};

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
pub fn parse_crs_installer(crs_path: &Path) -> Result<Vec<CursorMapping>> {
    // I assume paths are relative to the CRS file? Wouldn't
    // make sense otherwise but this format has no spec :P
    let parent = crs_path
        .parent()
        .ok_or_else(|| anyhow!("no parent for crs_path={}", crs_path.display()))?;

    let crs = Ini::new()
        .read(fs::read_to_string(crs_path)?)
        .map_err(|e| anyhow!("failed to read crs, error e={e}"))?;

    let mut mappings = Vec::with_capacity(CursorType::NUM_VARIANTS);

    for (section, value) in crs {
        let Some(r#type) = section_to_type(&section) else {
            bail!("unexpected section in crs file (please report), section={section}");
        };

        let Some(relative) = value.get("path").and_then(Option::as_ref) else {
            warn!("skipping section_name={section}");
            continue;
        };

        let path = parent.join(relative);

        mappings.push(CursorMapping { r#type, path });
    }

    Ok(mappings)
}
