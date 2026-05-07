//! Groups modules that handle file formats, such as ANI and Xcursor.

use std::path::PathBuf;

use crate::themes::theme::CursorType;

pub mod ani;
pub mod crs;
pub mod inf;
pub mod xcursor;

/// Intermediate representation when going from parsing
/// cursor mappings/roles to parsing the cursors themselves.
///
/// No clue what I just said there.
///
/// Reasons why this exists:
///
/// - An entire struct would be, as they say, "doing too much"
/// - Mixing cursor parsing and parsing of mappings is bad, methinks
/// - I don't want to type `(PathBuf, CursorType)` all the time
pub(super) type CursorMapping = (PathBuf, CursorType);
