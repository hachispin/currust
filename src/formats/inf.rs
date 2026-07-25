//! Parses INF installer files for cursor themes.

use crate::{
    themes::theme::{CursorMapping, CursorType},
    warn,
};

use std::{collections::HashMap, fs, path::Path};

use anyhow::{Context, Result, anyhow, bail};
use configparser::ini::Ini; // inf is an "ini-like" format

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
/// In INF installer files, the `Scheme.Reg` section is usually (but not always!) like this
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

    let addreg = inf
        .get("defaultinstall")
        .ok_or_else(|| anyhow!("no defaultinstall section found"))?
        .get("addreg")
        .ok_or_else(|| anyhow!("no addreg key found in defaultinstall"))?
        .as_ref()
        .ok_or_else(|| anyhow!("no value for addreg key"))?;

    // find the right registry entries (the ones we can parse)
    // https://github.com/quantum5/win2xcur/blob/c8a390b79456a45104fe42133b9d7eb4ce7c8638/win2xcur/parser/inf.py#L47-L50
    let scheme = addreg.split(',')
        .filter_map(|k| inf.get(&k.to_ascii_lowercase()))
        .flat_map(|v| v.keys())
            .find(|k|
                k.starts_with(r#"hkcu,"control panel\cursors\schemes","#) ||
                k.starts_with(r#"hklm,"software\microsoft\windows\currentversion\control panel\cursors\schemes","#)
        )
        .ok_or_else(|| anyhow!("couldn't find cursor mappings"))?;

    let subs = inf.get("strings");
    let expanded_reg = expand_scheme(scheme, subs)?;
    let mut reg_info = expanded_reg.split(',');

    reg_info.next(); // root key, e.g., hkcu, hklm
    reg_info.next(); // path

    let name = reg_info
        .next()
        .ok_or_else(|| anyhow!("couldn't parse theme name; reg_info doesn't have enough info"))?
        .strip_prefix('"') // refrain from trim_matches; only one quote should be removed
        .and_then(|n| n.strip_suffix('"'))
        .ok_or_else(|| anyhow!("expected theme name to be quoted"))?
        .to_string();

    reg_info.next(); // flags

    let mut paths: Vec<_> = reg_info
        .map(|s| {
            s.rsplit_once('\\')
                .ok_or_else(|| anyhow!("failed to extract filename from path, s={s}"))
                .map(|s| s.1)
        })
        .collect::<Result<_>>()?;

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
                    // let's just assume it's a DIRID and leave it :)
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
                        path: $root.join(concat!("Neuro ", $filename_suffix, ".ani")),
                    },
                )+
            ]}
        }

        let theme_dir = Path::new(from_root!("/testing/fixtures/neuro"));
        let inf_path = theme_dir.join("Install.inf");
        let (theme_name, mappings) = parse_inf_installer(&inf_path, theme_dir).unwrap();
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
