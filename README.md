# currust - a cursor converter
[![Release](https://github.com/hachispin/currust/actions/workflows/release.yml/badge.svg)](https://github.com/hachispin/currust/actions/workflows/release.yml)
[![crates.io](https://img.shields.io/crates/v/currust.svg)](https://crates.io/crates/currust)

A tool written in Rust to convert cursors between Windows and Linux. Specifically,
converting from the CUR/ANI format to the Xcursor format (plus some other features).

There are quite a few CLI tools that already exist for this purpose. You can see a comparison
in [COMPARISONS.md](COMPARISONS.md) that I've tried to keep neutral if you need help deciding.

## Installation

There are currently two supported methods of installation:

- download the binaries on the releases page (recommended)
- build from crates with `cargo install currust` (requires `cargo`)

## Usage

The intended use-case of this tool is to convert a Windows _cursor theme_ to Linux. A _cursor theme_
is a directory that contains some cursors, along with an installer file that uses the INF format.

You can convert a cursor theme as such:

```bash
currust ./my-cursor-theme
```

This converts the theme and writes the produced X11 theme (which is a directory) in the current
directory. Add the `--out` (or `-o` for short) argument to place it in the specified path.

```bash
currust ./my-cursor-theme -o ./please/go/here/instead
```

Cursor themes on Windows can be scaled by Windows itself. Unfortunately, this feature doesn't
exist on most Linux distributions, so Xcursor themes have to include their own size variations.

The `--scale-to` argument is available to provide some scale factors, along
with `--scale-with` to provide a scaling algorithm to use (default: Lanczos3).

Note that this increases the size of the resulting cursor theme.

```text
currust ./my-cursor-theme --scale-to 1.5 2 3 --scale-with mitchell
```

Afterwards, move the converted theme to the system-wide `/usr/share/icons` or the local
`~/.local/share/icons`. Note that other valid locations do exist, according to the [specification](https://specifications.freedesktop.org/icon-theme/latest/#directory_layout).
Switching to this cursor theme depends on your distribution, so just (kindly) look it up.

For more information on other commands and possible usages, view the help text:

```text
./currust -h      # Summarised help text
./currust --help  # Detailed help text
```

## Goals

All the baseline goals I had for this project are complete, so this is more akin to
a "planned/future features" section. Note that not everything here may be added.

- [ ] Publish or otherwise for usage with `cargo` and package managers
- [ ] Conversion from X11 cursors to Windows cursors (i.e, the other way around)
- [ ] [SVG cursor themes](https://blog.vladzahorodnii.com/2024/10/06/svg-cursors-everything-that-you-need-to-know-about-them) for KDE Plasma
- [ ] hyprcursor (cursor format for hyprland) support
      
---

The name ("currust") comes from a portmanteau of "cursor" and "Rust".
