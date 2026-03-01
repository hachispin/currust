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
    let inf_string = fs::read_to_string(inf_path)?;

    let inf = Ini::new()
        .read(inf_string)
        .map_err(|e| anyhow!("failed to read inf, error e={e}"))?;

    let reg = inf
        .get("scheme.reg")
        .ok_or_else(|| anyhow!("no scheme.reg found in inf"))?;

    if reg.keys().len() != 1 {
        bail!(
            "expected reg to have one key, instead has {} (reg={:?})",
            reg.keys().len(),
            reg
        );
    }

    if reg.values().next() != Some(&None) {
        bail!(
            "expected no value (None) for reg, instead got {:?}",
            reg.values().next()
        )
    }

    let reg = reg
        .keys()
        .next()
        .ok_or_else(|| anyhow!("no key for scheme.reg found in inf"))?;

    let subs = inf.get("strings");
    let expanded_reg = expand_reg(reg, subs)?;
    let mut reg_info = expanded_reg.split(',');
    let hkcu = reg_info.next();
    let _ = reg_info.next(); // sometimes blank, sometimes 0x00010000...?

    if !hkcu.is_some_and(|s| s.eq_ignore_ascii_case("hkcu")) {
        bail!("expected 'hkcu' for first reg_info value, instead got {hkcu:?}");
    }

    let name = reg_info
        .next()
        .ok_or_else(|| anyhow!("couldn't parse theme name; reg_info doesn't have enough info"))?
        .strip_prefix('"')
        .unwrap_or_default()
        .strip_suffix('"')
        .map(str::to_string)
        .ok_or_else(|| anyhow!("expected theme name to be quoted"))?;

    reg_info.next(); // unused field

    let mut paths: Vec<_> = reg_info
        .map(|s| {
            s.rsplit_once('\\')
                .ok_or_else(|| anyhow!("failed to extract filename from path, s={s}"))
                .map(|s| s.1)
        })
        .collect::<Result<_>>()?;

    if paths.len() != 17 {
        // maybe upgrade to error?
        eprintln!(
            "[warning] expected 17 paths, instead got {} paths",
            paths.len()
        );
    }

    let end = paths.len() - 1;
    paths[end] = paths[paths.len() - 1]
        .strip_suffix('"')
        .ok_or_else(|| anyhow!("expected closing quotation for paths, didn't find it"))?;

    let mappings: Vec<_> = paths
        .into_iter()
        .zip(0..15)
        .map(|(p, i)| CursorMapping {
            r#type: index_to_cursor_type(i),
            path: theme_dir.join(p),
        })
        .collect();

    Ok((name, mappings))
}

/// Helper function for [`parse_inf_installer`].
///
/// The index should be offsets relative to the first cursor in `Scheme.Reg`.
#[rustfmt::skip]
const fn index_to_cursor_type(index: usize) -> CursorType {
    use CursorType::*;

    match index {
         0 => Arrow,          1 => Help,
         2 => LeftPtrWatch,   3 => Watch,
         4 => Crosshair,      5 => Text,
         6 => Pencil,         7 => Forbidden,
         8 => NsResize,       9 => EwResize,
        10 => NwseResize,    11 => NeswResize,
        12 => Move,          13 => CenterPtr,
        14 => Hand,           _ => unreachable!(),

        // 15/16 are person and pin, which do not 
        // have (commonly-used) xcursor equivalents
    }
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
            .or_else(|| if sub_key == "%%" { Some("%") } else { None })
            .or_else(|| {
                if sub_key.chars().all(|c| c.is_ascii_digit() || c == '%') {
                    // let's just assume it's a DIRID and leave it :)
                    Some(&sub_key)
                } else {
                    None
                }
            })
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
