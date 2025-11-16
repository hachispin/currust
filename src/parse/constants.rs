//! Stores constants, such as file headers and important offsets.
//!
//! Note: Windows formats (incl. `.cur`) are generally little-endian.

// https://en.wikipedia.org/wiki/ICO_(file_format)#NEWHEADER_structure
pub const CUR_MAGIC: [u8; 4] = [0x00, 0x00, 0x02, 0x00];

// ICONDIR offsets:
// https://en.wikipedia.org/wiki/ICO_(file_format)#ICONDIR_structure

nofmt::pls! {

pub const NUM_IMAGES_OFFSET         : usize = 4;
pub const ICONDIRENTRY_ARRAY_OFFSET : usize = 6;

}

// ICONDIRENTRY sizes and offsets:
// https://en.wikipedia.org/wiki/ICO_(file_format)#ICONDIRENTRY_structure
//
// `OFFSET_IMG_DATA_OFFSET` may sound confusing, it holds the
// the offset of BMP/PNG data from the beginning of the CUR file.
//
// i.e, blob[OFFSET_IMG_DATA_OFFSET] is where image data starts.
//
// The same applies for `SIZE_IMG_BYTE_SIZE`, it's the size of the
// data type (e.g, u32) that stores the number of bytes in the image.

nofmt::pls! {

pub const ICONDIRENTRY_SIZE : usize = 16;

pub const IMG_WIDTH_SIZE    : u32 = 1;
pub const IMG_HEIGHT_SIZE   : u32 = 1;
pub const COLOR_COUNT_SIZE  : u32 = 1;
// [Reserved]               : u32 = 1;
pub const HOTSPOT_X_SIZE    : u32 = 2;
pub const HOTSPOT_Y_SIZE    : u32 = 2;
pub const SIZE_IMG_SIZE     : u32 = 4;

pub const IMG_WIDTH_OFFSET      : usize = 0;
pub const IMG_HEIGHT_OFFSET     : usize = 1;
pub const COLOR_COUNT_OFFSET    : usize = 2;
// [Reserved]                   : usize = 3;
pub const HOTSPOT_X_OFFSET      : usize = 4;
pub const HOTSPOT_Y_OFFSET      : usize = 6;
pub const IMG_SIZE_OFFSET       : usize = 8;
pub const OFFSET_IMG_DATA_OFFSET: usize = 12;

}
