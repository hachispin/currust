//! Parses CRS files from RW Cursor editor.
//!
//! Pretty sure CRS is just TOML with required sections.

use crate::{cursors::cursor_role::CursorRole, themes::theme::CursorMapping, warn};

use std::{fs, path::Path};

use anyhow::{Result, anyhow, bail};
use configparser::ini::Ini;

/// Section names in CRS files.
fn section_to_role(section: &str) -> Option<CursorRole> {
    use CursorRole::*;

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

    let mut mappings = Vec::with_capacity(CursorRole::NUM_VARIANTS);

    for (section, value) in crs {
        let Some(role) = section_to_role(&section) else {
            bail!("unexpected section in crs file (please report), section={section}");
        };

        let Some(relative) = value.get("path").and_then(Option::as_ref) else {
            warn!("skipping section_name={section}");
            continue;
        };

        let path = parent.join(relative);

        mappings.push(CursorMapping { role, path });
    }

    Ok(mappings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::from_root;

    #[test]
    fn parses_standalone_crs() {
        use CursorRole::*;

        let fixture = Path::new(from_root!("/testing/fixtures/idk-either.crs"));
        let parent = fixture.parent().unwrap();
        let mappings = parse_crs_installer(fixture).unwrap();
        let expected = [
            (Arrow, "idkeither.cur"),
            (CenterPtr, "idkeitheralt.cur"),
            (Watch, "idkeitherbusy.cur"),
            (Help, "idkeitherhelp.cur"),
            (Hand, "idkeitherlink.cur"),
            (Move, "idkeithermove.cur"),
            (Forbidden, "idkeithernein.cur"),
            (Pencil, "idkeitherpencil.cur"),
            (Crosshair, "idkeitherprecision.cur"),
            (NsResize, "idkeitherresize1.cur"),
            (EwResize, "idkeitherresize2.cur"),
            (NwseResize, "idkeitherresize3.cur"),
            (NeswResize, "idkeitherresize4.cur"),
            (Text, "idkeithertext.cur"),
            (LeftPtrWatch, "idkeitherworking.cur"),
        ];

        assert_eq!(mappings.len(), expected.len());

        for (role, filename) in expected {
            let mapping = CursorMapping {
                role,
                path: parent.join(filename),
            };
            assert!(mappings.contains(&mapping), "missing {mapping:?}");
        }
    }

    #[test]
    fn parses_crs_with_omitted_roles() {
        let directory = tempfile::tempdir().unwrap();
        let fixture = directory.path().join("minimal.crs");
        fs::write(&fixture, "[Default]\nPath=cursor.cur\n\n[Help]\n").unwrap();

        assert_eq!(
            parse_crs_installer(&fixture).unwrap(),
            [CursorMapping {
                role: CursorRole::Arrow,
                path: directory.path().join("cursor.cur"),
            }]
        );
    }
}
