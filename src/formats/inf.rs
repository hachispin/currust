//! Parses INF installer files for cursor themes.

use crate::themes::theme::CursorType;

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Result, anyhow, bail};
// inf isn't exactly ini but it's close
// enough to not produce parsing errors
use configparser::ini::Ini;

/// Cursor mappings stored in INF files.
pub struct CursorMapping {
    /// Semantic role of cursor.
    pub r#type: CursorType,
    /// Full path to (expected) cursor.
    pub path: PathBuf,
}

/// Attempts to parse `inf_path` as an installer file for a cursor theme.
///
/// Returns the tuple (`theme_name`, `cursor_mappings`).
///
/// ## Errors
///
/// A lot.
///
/// ## Implementation details
///
/// In INF installer files, the `Scheme.Reg` section should be formatted as such:
///
/// ```text
/// ; note that this is pseudocode, this isn't a valid inf file
///
/// ; this section always starts like this
/// HKCU,"Control Panel\Cursors\Schemes","theme_name",<IGNORE>,
///
/// ; the cursors are always ordered like this
/// ; sometimes they're variables, sometimes not
/// "pointer,help,work,busy,cross,text,hand,unavailable,
/// vert,horz,dgn1,dgn2,move,alternate,link,pin,person"
/// ```
pub fn parse_inf_installer(
    inf_path: &Path,
    theme_dir: &Path,
) -> Result<(String, Vec<CursorMapping>)> {
    let inf_path_display = inf_path.display();
    let inf_string = fs::read_to_string(inf_path)?;

    let inf = Ini::new()
        .read(inf_string)
        .map_err(|e| anyhow!("failed to read {inf_path_display}: {e}"))?;

    let reg = inf
        .get("scheme.reg")
        .ok_or_else(|| anyhow!("no scheme.reg found in {inf_path_display}"))?;

    debug_assert_eq!(reg.keys().len(), 1);
    debug_assert_eq!(reg.values().next(), Some(&None));

    let reg = reg
        .keys()
        .next()
        .ok_or_else(|| anyhow!("no key for scheme.reg found in {inf_path_display}"))?;

    let subs = inf.get("strings");
    let expanded_reg = expand_reg(reg, subs)?;

    // skip hkcu, control panel
    let mut reg_info = expanded_reg.split(',').skip(2);

    // TODO: remove unwraps around here
    let name = reg_info.next().unwrap().to_string();
    reg_info.next(); // unused field

    let mut paths: Vec<_> = reg_info.map(|s| s.rsplit_once('\\').unwrap().1).collect();
    let end = paths.len() - 1;
    paths[end] = paths[paths.len() - 1].strip_suffix('"').unwrap();

    let mappings: Vec<_> = paths
        .into_iter()
        .zip(0..15)
        .map(|(p, i)| CursorMapping {
            r#type: index_to_cursor_type(i).unwrap(),
            path: theme_dir.join(p),
        })
        .collect();

    Ok((name, mappings))
}

/// Helper function for [`parse_inf_installer`].
///
/// The index should be offsets relative to the first cursor in `Scheme.Reg`.
fn index_to_cursor_type(index: usize) -> Option<CursorType> {
    use CursorType::*;

    Some(match index {
        0 => Arrow,
        1 => Help,
        2 => LeftPtrWatch,
        3 => Watch,
        4 => Crosshair,
        5 => Text,
        6 => Pencil,
        7 => Forbidden,
        8 => NsResize,
        9 => EwResize,
        10 => NwseResize,
        11 => NeswResize,
        12 => Move,
        13 => CenterPtr,
        14 => Hand,
        15 | 16 => return None, // person/location select have no equivalent
        _ => panic!("unexpected cursor type at index={index}"),
    })
}

/// Helper function for [`parse_inf_installer`]. This expands `Scheme.Reg` if needed.
///
/// NOTE: this does **not** handle nested substitutions,
///       but there should be no need for that. Hopefully.
fn expand_reg(reg: &str, subs: Option<&HashMap<String, Option<String>>>) -> Result<String> {
    let Some(subs) = subs else {
        let empty: HashMap<String, String> = HashMap::new();
        return expand(reg, &empty);
    };

    let subs: HashMap<_, _> = subs
        .iter()
        .filter_map(dequote_value)
        .map(|(k, v)| {
            let mut k_var = k;
            k_var.insert(0, '%');
            k_var.push('%');
            (k_var, v)
        })
        .collect();

    expand(reg, &subs)
}

/// Helper function for [`expand_reg`] for removing the outer pair of quotes.
///
/// This is because [`configparser`] takes _everything_ as a string,
/// for example: `key = "value"` means `config["key"] == "\"value\""`.
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

/// Expands percent-delimited values using `subs` as a lookup table.
fn expand(value: &str, subs: &HashMap<String, String>) -> Result<String> {
    let mut expanded_value = value.to_string();
    let value_ilen = i64::try_from(value.len())?;
    let sub_ranges: Vec<_> = value.match_indices('%').map(|(i, _)| i).collect();

    if !sub_ranges.len().is_multiple_of(2) {
        bail!(
            "unclosed delimiter in value={value}: the number of found \
            percentage (%) delimiters (len()={}) aren't a multiple of 2",
            sub_ranges.len()
        );
    }

    for &[start, end] in sub_ranges.as_chunks::<2>().0 {
        let sub_key = value[start..=end].to_string();
        let sub_value = subs
            .get(&sub_key)
            .map(String::as_str)
            .or_else(|| expand_dirids(&sub_key))
            .ok_or_else(|| {
                anyhow!("no substitution exists for sub_key={sub_key} for value={value}")
            })?;

        let offset = i64::try_from(expanded_value.len())? - value_ilen;
        let (istart, iend) = (i64::try_from(start)?, i64::try_from(end)?);
        let (start, end) = (
            usize::try_from(istart + offset)?,
            usize::try_from(iend + offset)?,
        );

        expanded_value.replace_range(start..=end, sub_value);
    }

    Ok(expanded_value)
}

/// Helper for [`expand`] for system-defined variables.
// TODO: maybe consider adding more dirids if seen
fn expand_dirids(key: &str) -> Option<&str> {
    // because hashmaps are annoying const-wise
    const LEN: usize = 2;

    const SYS_KEYS: [&str; LEN] = ["%%", "%10%"];
    const SYS_VALUES: [&str; LEN] = ["%", "C:\\WINDOWS"];

    let idx = SYS_KEYS.iter().position(|k| *k == key)?;
    SYS_VALUES.get(idx).copied()
}
