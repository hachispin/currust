//! Contains the [`CursorImage`] struct, which is used
//! as a medium between Windows and Linux cursors.

pub struct CursorImage {
    pub width: u32,
    pub height: u32,
    pub hotspot_x: u32,
    pub hotspot_y: u32,
    pub rgba: Vec<u8>,
}
