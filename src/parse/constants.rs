//! Stores constants, such as file headers.
//! 
//! Note: Windows formats are generally little-endian.

// https://en.wikipedia.org/wiki/ICO_(file_format)#NEWHEADER_structure
pub const CUR_MAGIC: [u8; 4] = [0x00, 0x00, 0x02, 0x00];
