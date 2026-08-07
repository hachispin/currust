# currust - a cursor converter

[![Release](https://github.com/hachispin/currust/actions/workflows/release.yml/badge.svg)](https://github.com/hachispin/currust/actions/workflows/release.yml)
[![crates.io](https://img.shields.io/crates/v/currust.svg)](https://crates.io/crates/currust)

A tool written in Rust to convert cursor formats between Windows and Linux. Specifically,
converting from the CUR/ANI format to the Xcursor format (plus some other features).

Once installed, you can run `currust --help` (or `currust -h` for a
shorter summary) to see information regarding available options and flags.

## Installation

There are currently two supported methods of installation:

- download the binaries on the [releases page](https://github.com/hachispin/currust/releases/latest) (recommended)
- build from crates.io with `cargo install currust`

## Usage (automatic)

The primary use-case of currust is to convert a Windows _cursor theme_
to Linux. A _cursor theme_ is simply a collection of cursor files
usually accompanied by an installer file in the INF or CRS format.

If the cursor theme being converted doesn't include an
installer file, read the [manual usage section](#usage-manual).

A Windows cursor theme can be converted as such:

```bash
$ currust ./my-cursor-theme/installer.inf
# Or:
$ currust ./my-other-cursor-theme/installer.crs
```

This converts the theme and writes the produced X11 theme (which is a directory) in the current
directory. Add the `--out` (or `-o` for short) argument to place it in the specified path.

```bash
$ currust ./my-cursor-theme/installer.inf -o ./please/go/here/instead
```

Cursor themes on Windows can be scaled by Windows itself. Unfortunately, this feature
doesn't exist anywhere on Linux from what I know, so Xcursor themes have to include
their own size variations if the provided sizes (usually just 32x32) is too small.

currust has support for both upscaling and downscaling cursors.

The `--scale-to` argument is used to specify what scale factors to scale to,
along with `--scale-with` to specify a scaling algorithm to use (default: box).

> [!TIP]
> The default algorithm (box) is meant for pixel-art. If you want smoother
> scaling, specify that with, e.g., Lanczos3 using `--scale-with lanczos`.

For example, to scale my-cursor-theme to 0.5x, 2x and 3x using Mitchell:

```bash
$ currust ./my-cursor-theme/installer.inf --scale-to 0.5 2 3 --scale-with mitchell
```

Note that this increases the size of the resulting cursor theme.

## Usage (manual)

The cursor theme being converted may lack an installer file or have one in an unsupported format.
To convert, pass the `--manual` flag, where a guided installation process will occur. Note that as
of now, this configuration isn't saved--this must be done each time the theme is to be converted.

The `--manual` flag must also be accompanied with the paths of cursors to use.
Directories are expanded (non-recursively) to the cursor files they contain, providing
a similar function to _globbing_ on shells that don't support it (e.g., `pwsh`).

<details>
<summary>Manual conversion</summary>

A prompt will appear for each cursor in the theme that needs to be converted. A checkmark
will appear on cursors you've already selected (though, you can re-select them if needed).

You can move up and down the list using the `k` and `j` keys respectively.

In case the (admittedly rudimentary) emoji/glyph visual representations of
the cursor aren't enough, here's a convenient reference image you can use.

![Windows cursors role reference](./windows-cursors.png)

```bash
$ currust --manual Cursors  # Directory with cursor files
? Select the file representing 'Help'.
Description: a question mark, may include a pointer
Used when: hovering over something that has a tooltip
Names: help select, help
Looks like: [( ↖️ or  ) + ?] or ? ›
  'Alternate Select.ani'
  'Busy.ani' ✓✓  # n checkmark(s) ⇒ already selected n time(s)
  'Diagonal Resize 1.ani'
  'Diagonal Resize 2.ani'
  'Handwriting.ani'
❯ 'Help Select.ani'  # Selector: move up/down with k/j; enter to confirm
  'Horizontal Resize.ani'
  'Link Select.ani' ✓
  'Location Select.ani'
  'Move.ani'
  'Normal Select.ani' ✓
  'Person Select.ani'
  'Precision Select.ani'
  'Text Select 2.ani'
  'Text Select.ani'
  'Unavailable.ani'
  'Vertical Resize.ani'
  'Working In Background.ani'
```

</details>

## Changing and installing the cursor theme

Afterwards, move the converted theme to the local `~/.icons`. Any location specified in
[here](https://specifications.freedesktop.org/icon-theme/latest/#directory_layout) should work.

> [!WARNING]
> Placing cursor themes in the system-wide `/usr/share/icons`
> isn't recommended due to the extra permissions required.

You can then set the converted theme as your cursor depending on your DE:

- KDE: `plasma-apply-cursortheme <theme_name> --size <size>`
- GNOME: `gsettings set org.gnome.desktop.interface cursor-theme <theme_name> && gsettings set org.gnome.desktop.interface cursor-size <size>`
- Hyprland: `hyprctl setcursor <theme_name> <size>`

The theme name is the same as the name of the converted
theme directory. If your DE isn't listed, look it up.

<details>
<summary>Example installation on KDE</summary>

```bash
$ tree  # Expected cursor theme layout (as input)
.
└── [The Herta Cursor ver.2.0.0]
    ├── 01-Normal.ani
    ├── 02-Link.ani
    ├── 03-Loading.ani
    ├── 04-Help.ani
    ├── 05-Text Select Alt.ani
    ├── 05-Text Select.ani
    ├── 06-Handwriting.ani
    ├── 07-Precision.ani
    ├── 08-Unavailable.ani
    ├── 09-Location Select.ani
    ├── 10-Person Select.ani
    ├── 11-Vertical Resize.ani
    ├── 12-Horizontal Resize.ani
    ├── 13-Diagonal Resize 1.ani
    ├── 14-Diagonal Resize 2.ani
    ├── 15-Move.ani
    ├── 16-Alternate Select.ani
    ├── [Changelog].txt
    └── Installer.inf  # ← A supported installer file!

2 directories, 19 files

$ currust \[The\ Herta\ Cursor\ ver.2.0.0\]/Installer.inf --scale-to 5 -o ~/.icons

$ plasma-apply-cursortheme ~/.icons/The\ Herta\ Cursor\ ver\ 2.0.0
Successfully applied the mouse cursor theme The Herta Cursor ver 2.0.0 to your current Plasma session
```

</details>

## About Windows

For Windows, conversion still works. The only limitation is the creation of symlinks, which
can be done through a created bash script once the converted theme is on a Linux system.

Note that the script (once on Linux) will need to be given execution permissions with `chmod +x`.

## Next steps?

Possible tasks to consider doing. May not be done.

- [x] Publish or otherwise for usage with `cargo` and package managers
- [ ] Replace bash script generation by exporting to tar.gz
- [ ] Conversion from Xcursor to ANI/CUR (i.e, the other way around)
- [ ] [SVG cursor themes](https://blog.vladzahorodnii.com/2024/10/06/svg-cursors-everything-that-you-need-to-know-about-them) for KDE Plasma
- [x] ~~hyprcursor (cursor format for hyprland) support~~
      → covered by [hyprcursor-util](https://github.com/hyprwm/hyprcursor/tree/main/hyprcursor-util)
- [x] Have a guided installation process for themes with no installer file

---

The name ("currust") comes from a portmanteau of "cursor" and "Rust".

Any time I refer to "Linux", it may be better thought of as "Linux distributions" or "Xcursor".
