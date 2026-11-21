//! Stores enum representing the semantic role of a cursor.

use documented::DocumentedVariants;

/// Represents the possible cursors that exist in both Windows and Linux (X11).
///
/// Some cursors, such as `Crosshair`, have symlinks to Xcursors
/// that aren't _exactly_ the same, such as `color-picker`.
#[derive(Debug, PartialEq, Eq, Clone, DocumentedVariants)]
pub enum CursorRole {
    // using https://github.com/khayalhus/win2xcur-batch/blob/main/map.json
    // NOTE: documentation here is displayed to users in manual installs.
    /// Description: an arrow pointing to the top-left; the default cursor
    /// Used when: nothing is happening
    /// Names: normal select, normal, pointer, arrow
    /// Looks like: ↖️ or 
    Arrow,
    /// Description: a hand pointing upwards
    /// Used when: hovering over a link or anything that you can click
    /// Names: link select, link, hand
    /// Looks like: 👆 or 
    Hand,
    /// Description: a spinning wheel or a spinner
    /// Used when: something is loading
    /// Names: busy, wait, waitcursor
    /// Looks like: 🔃 or 
    Watch,
    /// Description: a pointer with a spinning wheel or spinner
    /// Used when: something is loading in the background
    /// Names: working in background, work, appstarting
    /// Looks like: ( ↖️ and 🔃 ) or (  and  )
    LeftPtrWatch,
    /// Description: a question mark, may include a pointer
    /// Used when: hovering over something that has a tooltip
    /// Names: help select, help
    /// Looks like: [( ↖️ or  ) + ?] or ?
    Help,
    /// Description: an I-beam or a serifed I
    /// Used when: hovering over a text input field
    /// Names: text select, text, ibeam
    /// Looks like: ⌶ or 𝙸 or エ or 
    Text,
    /// Description: a pencil or pen
    /// Used when: drawing
    /// Names: handwriting, hand
    /// Looks like: ✏️ or 
    Pencil,
    /// Description: a crosshair, usually drawn as a plus
    /// Used when: taking screenshots
    /// Names: precision select, precision, crosshair, cross
    /// Looks like: + or ➕ or ✛
    Crosshair,
    /// Description: a slashed circle (a "no symbol") or crossbones
    /// Used when: indicating something can't be clicked/dragged into
    /// Names: unavailable, no
    /// Looks like: 🚫 or ☠️
    Forbidden,
    /// Description: a double-sided vertical arrow
    /// Used when: resizing something vertically
    /// Names: vertical resize, vert, sizens
    /// Looks like: ↕ or 
    NsResize,
    /// Description: a double-sided horizontal arrow
    /// Used when: resizing something horizontally
    /// Names: horizontal resize, horz, sizewe
    /// Looks like: ↔ or 
    EwResize,
    /// Description: a double-sided diagonal arrow taking the top-left and bottom-right corners
    /// Used when: resizing something from the top-left or bottom-right corner
    /// Names: diagonal resize 1, dgn1, sizenwse
    /// Looks like: ⤡ or 󰹵
    NwseResize,
    /// Description: a double-sided diagonal arrow taking the top-right and bottom-left corners
    /// Used when: resizing something from the top-right or bottom-left corner
    /// Names: diagonal resize 2, dgn2, sizenesw
    /// Looks like: ⤢ or 󰹷
    NeswResize,
    /// Description: four arrows pointing up, down, left and right joined together
    /// Used when: moving/dragging something
    /// Names: move, sizeall
    /// Looks like: ( ↔ and ↕ ) or ✥ or 󰁁
    Move,
    /// Description: an arrow facing upwards
    /// Used when: the normal cursor would be disruptive
    /// Names: alternate select, alt, uparrow
    /// Looks like: ↑ or 
    CenterPtr,
}

impl CursorRole {
    pub const NUM_VARIANTS: usize = 15;
    pub const VARIANTS: [Self; Self::NUM_VARIANTS] = [
        Self::Arrow,
        Self::Hand,
        Self::Watch,
        Self::LeftPtrWatch,
        Self::Help,
        Self::Text,
        Self::Pencil,
        Self::Crosshair,
        Self::Forbidden,
        Self::NsResize,
        Self::EwResize,
        Self::NwseResize,
        Self::NeswResize,
        Self::Move,
        Self::CenterPtr,
    ];
}
