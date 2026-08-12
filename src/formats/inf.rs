//! Parses INF installer files for cursor themes.

use crate::{
    themes::theme::{CursorMapping, CursorType},
    warn,
};

use std::{collections::HashMap, fs, path::Path};

use anyhow::{Context, Result, anyhow, bail};
use configparser::ini::{Ini, IniDefault};
use csv::{ReaderBuilder, StringRecord}; // inf is an "ini-like" format

fn inf_new() -> Ini {
    let mut defaults = IniDefault::default();

    // non-exhaustive so we gotta do this
    defaults.comment_symbols = vec![';'];
    defaults.delimiters = vec!['='];

    Ini::new_from_defaults(defaults)
}

/// [`fs::read_to_string`] with UTF16 considerations.
///
/// Follows the BOM if present, otherwise, parses as
/// lossy UTF-8 to account for ASCII and ANSI code pages.
fn read_to_string_utf16(path: &Path) -> Result<String> {
    let mut bytes = fs::read(path)?;

    Ok(match bytes.as_slice() {
        [0xff, 0xfe, rest @ ..] => String::from_utf16le(rest)?,
        [0xfe, 0xff, rest @ ..] => String::from_utf16be(rest)?,
        [0xef, 0xbb, 0xbf, ..] => {
            bytes.drain(0..3);
            String::from_utf8(bytes)?
        }
        _ => String::from_utf8_lossy_owned(bytes),
    })
}

/// Reads a single record and splits fields following CSV behaviour.
///
/// Also trims.
fn split_csv(record_str: &str) -> Result<StringRecord> {
    let mut rdr = ReaderBuilder::new()
        .has_headers(false)
        .from_reader(record_str.as_bytes());

    let Some(record) = rdr.records().next() else {
        bail!("no records found")
    };

    let mut record = record?;
    record.trim();

    if rdr.records().next().is_some() {
        warn!("more than one record found, returning first");
    }

    Ok(record)
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
/// First, parse the order stored in `Scheme.Reg` to get each cursors' semantic role. Since the
/// paths stored there are the _destination_ paths, we need to get the _source_ paths (the ones
/// relative to the INF). This can be done by reading the section(s) stored in `CopyFiles`).
///
/// Each entry in the aforementioned section will look like this:
///
/// `destination-file-name[,[source-file-name][,[unused][,flag]]]`
///
/// To get the source file, we match the filename of the destination paths stored in `Scheme.Reg`.
/// If the source is omitted, both the source and destination filenames are the same.
///
/// ## INF `AddReg`
///
/// The `AddReg` directive refers to sections that add entries to the registry. The section
/// that installs the cursor theme is usually called `Scheme.Reg` and looks something like this:
///
/// ```text
/// ; note that this is pseudocode, this isn't a valid inf file
/// ; each entry follows the format:
/// ; reg-root,[subkey],[value-entry-name],[flags],[value][,[value]]
///
/// HKCU,"Control Panel\Cursors\Schemes","theme_name",<IGNORE>, \
/// "pointer,help,work,busy,cross,text,hand,unavailable, \
/// vert,horz,dgn1,dgn2,move,alternate,link,pin,person"
///
/// ; ^ the cursors are always ordered like this. sometimes
/// ; they're variables (in the `Strings` section), sometimes not
/// ```
pub fn parse_inf_installer(inf_path: &Path) -> Result<(String, Vec<CursorMapping>)> {
    let inf_string = read_to_string_utf16(inf_path)?;

    let parent = inf_path
        .parent()
        .ok_or_else(|| anyhow!("no parent for inf_path={}", inf_path.display()))?;

    let inf = inf_new()
        .read(inf_string)
        .map_err(|e| anyhow!("failed to read inf, error e={e}"))?;

    let defaultinstall = inf
        .get("defaultinstall")
        .ok_or_else(|| anyhow!("no defaultinstall section found"))?;

    let addreg_sections_string = defaultinstall
        .get("addreg")
        .and_then(|v| v.as_ref())
        .ok_or_else(|| anyhow!("no addreg found in defaultinstall"))?;

    let addreg_sections = split_csv(addreg_sections_string)?;

    // find the right registry entries (the ones we can parse)
    //
    // https://github.com/quantum5/win2xcur/blob/c8a390b79456a45104fe42133b9d7eb4ce7c8638/win2xcur/parser/inf.py#L47-L50
    let scheme: Vec<_> = addreg_sections
        .iter()
        .filter_map(|k| inf.get(&k.to_ascii_lowercase()))
        .flat_map(|v| v.keys())
        .filter(|k| k.contains("control panel\\cursors\\schemes"))
        .collect();

    let scheme = match scheme.as_slice() {
        [] => bail!("couldn't find any cursor mappings"),
        [entry] => entry,
        _ => bail!("more than one cursor mapping found: {scheme:?}"),
    };

    let subs = inf.get("strings");
    let expanded_reg = expand_scheme(scheme, subs)?;

    // reg-root,[subkey],[value-entry-name],[flags],[value][,[value]]
    let reg_info = split_csv(&expanded_reg)?;

    let (Some(name), Some(paths)) = (reg_info.get(2), reg_info.get(4)) else {
        bail!("expected cursor registry entry to have at least five fields, reg_info={reg_info:?}");
    };

    let name = name.to_string();
    let paths = split_csv(paths)?;

    // get filenames
    let dst_filenames: Vec<_> = paths
        .iter()
        .map(|p| {
            p.rsplit_once('\\')
                .ok_or_else(|| anyhow!("failed to extract filename from path, p={p}"))
                .map(|p| p.1.to_ascii_lowercase())
        })
        .collect::<Result<_>>()?;

    let src_paths = resolve_paths(&inf, defaultinstall, &dst_filenames)?;

    let mappings: Vec<_> = src_paths
        .into_iter()
        .zip(0..15)
        .filter(|(p, _)| !p.is_empty())
        .map(|(p, i)| CursorMapping {
            r#type: index_to_cursor_type(i),
            path: parent.join(p),
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

        // 15/16 are pin and person, which do not have (commonly-used) Xcursor equivalents
    }
}

/// Resolves destination paths to source paths.
fn resolve_paths(
    inf: &HashMap<String, HashMap<String, Option<String>>>,
    defaultinstall: &HashMap<String, Option<String>>,
    paths: &[String],
) -> Result<Vec<String>> {
    let copyfiles = defaultinstall
        .get("copyfiles")
        .cloned()
        .flatten()
        .ok_or_else(|| anyhow!("no copyfiles section"))?;

    let fields = split_csv(&copyfiles)?;

    // paths are coerced to lowercase because they're "keys" (from configparser's perspective).
    // this most likely causes some extra lookups, since the initial path most likely has
    // the correct casing. could be solved with Ini::new_cs() but probably isn't worth it.
    let mut mappings = HashMap::with_capacity(paths.len());

    for field in &fields {
        // TODO: Implement this later.
        if matches!(field.chars().next(), Some('@')) {
            bail!("unsupported '@' syntax in copyfiles");
        }

        let Some(section) = inf.get(&field.to_ascii_lowercase()) else {
            warn!("copyfiles refers to section '{field}', but the section is missing");
            continue;
        };

        for k in section.keys() {
            // destination-file-name[,[source-file-name][,[unused][,flag]]]
            let entry = split_csv(k)?;
            let mut entry = entry.iter().map(|f| f.replace('\\', "/"));

            let Some(dst) = entry.next() else {
                bail!("empty entry in section={field}")
            };

            if let Some(src) = entry.next()
                && !src.is_empty()
            {
                mappings.insert(dst, src);
            } else {
                mappings.insert(dst.clone(), dst);
            }
        }
    }

    let mut new = Vec::with_capacity(paths.len());

    for p in paths {
        new.push(
            mappings
                .get(p)
                .ok_or_else(|| anyhow!("missing mapping for {p}"))?
                .clone(),
        );
    }

    Ok(new)
}

/// Helper function for [`parse_inf_installer`].
///
/// This expands `Scheme.Reg` if needed.
fn expand_scheme(reg: &str, subs: Option<&HashMap<String, Option<String>>>) -> Result<String> {
    let Some(subs) = subs else {
        let empty: HashMap<String, String> = HashMap::new();
        return expand(reg, &empty).with_context(|| format!("for input reg={reg}"));
    };

    let subs: HashMap<_, _> = subs
        .iter()
        .filter_map(dequote_value)
        .map(|(k, v)| (format!("%{k}%"), v))
        .collect();

    expand(reg, &subs).with_context(|| format!("for input reg={reg}"))
}

/// Helper function for [`expand_scheme`] to remove the outer pair of quotes.
///
/// This is because [`configparser`] takes _everything_ as a string,
/// for example: `key = "value"` means `config["key"] == "\"value\""`.
fn dequote_value(entry: (&String, &Option<String>)) -> Option<(String, String)> {
    match entry {
        (k, Some(v)) => {
            let value = v.trim();

            let value = value
                .strip_prefix('"')
                .and_then(|value| value.strip_suffix('"'))
                .unwrap_or(value)
                .replace("\"\"", "\"");

            Some((k.clone(), value))
        }
        (k, None) => {
            // side effect but shhh
            warn!("key={k} has value None");
            None
        }
    }
}

/// Expands percent-delimited keys using `subs` as a lookup table.
///
/// `subs` keys must contain the delimiters (e.g., "%name%" => "hachispin").
/// This also does not expand recursively - hopefully there's no need for that.
fn expand(input: &str, subs: &HashMap<String, String>) -> Result<String> {
    let mut expanded = String::with_capacity(input.len());
    let mut chars = input.char_indices();

    while let Some((i, c)) = chars.next() {
        if c != '%' {
            expanded.push(c);
            continue;
        }

        let start = i;

        let Some((end, _)) = chars.find(|(_, c)| *c == '%') else {
            bail!("unclosed '%' delimiter starting at i={i}");
        };

        let key = &input[start..=end].to_ascii_lowercase();

        let value = subs
            .get(key)
            .map(String::as_str)
            .or_else(|| (key == "%%").then_some("%"))
            .or_else(|| {
                if key.chars().all(|c| c.is_ascii_digit() || c == '%') {
                    // let's just assume it's a DIRID and leave it, ok?
                    Some(key)
                } else {
                    None
                }
            })
            .ok_or_else(|| anyhow!("no substitution exists for key={key}"))?;

        expanded.push_str(value);
    }

    Ok(expanded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::from_root;

    /// Various tests for the [`expand`] function that should all return [`Ok`].
    #[test]
    fn expand_ok() {
        let mut subs = HashMap::new();
        subs.insert("%name%".to_string(), "hachispin".to_string());
        subs.insert("%mood%".to_string(), r"¯\_(ツ)_/¯".to_string());

        let value = "Hello! My name is %name%. Right now? I feel pretty meh. %mood%.";
        let expected = r"Hello! My name is hachispin. Right now? I feel pretty meh. ¯\_(ツ)_/¯.";
        assert_eq!(expand(value, &subs).unwrap(), expected);

        let value = "憧%name%";
        let expected = "憧hachispin";
        assert_eq!(expand(value, &subs).unwrap(), expected);

        let value = "21%%, 22%%, 23%%, 24%%, 25%%… 憧れ悩み　パンプアップ";
        let expected = "21%, 22%, 23%, 24%, 25%… 憧れ悩み　パンプアップ";
        assert_eq!(expand(value, &subs).unwrap(), expected);

        let value = "%%%name%%%%mood%%%";
        let expected = r"%hachispin%¯\_(ツ)_/¯%";
        assert_eq!(expand(value, &subs).unwrap(), expected);

        let value = "Madam Herta is a {'peerless gem','unrivaled genius','inimitable beauty'}.";
        assert_eq!(expand(value, &subs).unwrap(), value);
    }

    /// Various tests for the [`expand`] function that should all return [`Err`].
    #[test]
    fn expand_err() {
        let mut subs = HashMap::new();
        subs.insert("pitiful".to_string(), "so close!".to_string());

        let value = "One forgot to escape their delimiter. Only 50% of their body was found.";
        assert!(expand(value, &subs).is_err());

        let value = "The next escaped but forgot to insert the matching %value%.";
        assert!(expand(value, &subs).is_err());

        let value = "The last didn't read the documentation. How %pitiful%.";
        assert!(expand(value, &subs).is_err());
    }

    /// Golden file test for INF fixture.
    #[test]
    fn good_inf() {
        /// Macro for the mappings of this specific INF file.
        macro_rules! make_mappings {
            ($root:expr; $($variant:ident => $filename_suffix:literal),+ $(,)?) => {[
                $(
                    CursorMapping {
                        r#type: crate::themes::theme::CursorType::$variant,
                        path: $root.join(concat!("neuro ", $filename_suffix, ".ani")),
                    },
                )+
            ]}
        }

        let theme_dir = Path::new(from_root!("/testing/fixtures/neuro"));
        let inf_path = theme_dir.join("Install.inf");
        let (theme_name, mappings) = parse_inf_installer(&inf_path).unwrap();
        assert_eq!(theme_name, "Neuro-sama Cursor");

        let expected_mappings = make_mappings!(
            theme_dir;                   Arrow => "normal",
            Help => "help",              LeftPtrWatch => "work",
            Watch => "busy",             Crosshair => "precision",
            Text => "text",              Pencil => "hand",
            Forbidden => "unavailable",  NsResize => "vert",
            EwResize => "horz",          NwseResize => "dgn1",
            NeswResize => "dgn2",        Move => "move",
            CenterPtr => "alt",          Hand => "link",
        );

        assert_eq!(mappings, expected_mappings);
    }
}
