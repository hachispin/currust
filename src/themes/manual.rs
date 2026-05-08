use super::theme::CursorType;
use crate::themes::theme::CursorMapping;

use std::path::PathBuf;

use anyhow::{Result, anyhow};
use dialoguer::{Select, console::Term, theme::ColorfulTheme};
use documented::DocumentedVariants;

/// Asks the user a series of prompts to construct a theme manually.
///
/// This is used for when no installer file is present.
///
/// ## Errors
///
/// - any path in `cursor_paths` has no filename
/// - [`Select`] prompt fails (e.g., if user is not in a terminal)
pub(super) fn prompt_for_mappings(
    cursor_paths: &[PathBuf],
) -> Result<(String, Vec<CursorMapping>)> {
    let mut mappings = Vec::with_capacity(cursor_paths.len());
    let mut cursor_paths_display: Vec<_> = cursor_paths
        .iter()
        .map(|p| {
            p.file_name()
                .map(|f| format!("'{}'", f.display()))
                .ok_or_else(|| anyhow!("no file name for cursor path, p={}", p.display()))
        })
        .collect::<Result<_>>()?;

    for r#type in CursorType::VARIANTS {
        let prompt = format!(
            "Select the file representing '{:?}'.\n{}",
            r#type,
            r#type.get_variant_docs()
        );

        let chosen_index = Select::with_theme(&ColorfulTheme::default())
            .items(&cursor_paths_display)
            .with_prompt(prompt)
            .default(0)
            .report(false) // can get very messy as prompts are long
            .interact()?;

        let chosen = &mut cursor_paths_display[chosen_index];

        if !chosen.ends_with("(used)") {
            chosen.push_str(" (used)");
        }

        let path = cursor_paths[chosen_index].clone();
        mappings.push(CursorMapping { r#type, path });
    }

    eprint!("Enter a theme name: ");
    let name = Term::stderr().read_line()?;

    Ok((name, mappings))
}
