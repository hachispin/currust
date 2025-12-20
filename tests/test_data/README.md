# Cursors for unit tests

These are cursors used... for unit tests.

- The `*bpp.cur` files are self-explanatory.
- `*_rgba` files contain raw debug formatted RGBA bytes.
  - It's just one really long line so some editors might crash while opening it.
- Most of these cursors are sourced from rw-designer's [cursor library](https://www.rw-designer.com/cursor-library).

## Mind contributing?

If you have cursors with any of these properties, send me them!

- 24 bits per pixel colour depth
- Multiple stored images
- Non-standard image sizes (standard is 32x32)
- Negative height in `BITMAPINFOHEADER`

Don't send me cursors with these :(

- RLE-encoded pixel data, or any encoding other than uncompressed for that matter
- Negative width in `BITMAPINFOHEADER`
- FUBAR metadata (e.g, `color_planes` being non-zero)
- Any other weird, non-standard stuff
