# currust

A **work in progress** tool written in Rust to convert cursors between Windows and Linux
(specifically, the Xcursor format)

Unlike currently existing tools such as [win2xcur](https://github.com/quantum5/win2xcur), which
require a substantial amount of manual work, this project aims to be as **easy to use** as possible.
This also shouldn't require any external dependencies when released as a binary.

The UX point does get a bit weaker now since they've since added theme conversions (sort of)
after this project was made by parsing installer files. Oh well. &nbsp; ┐(￣ヘ￣)┌

## Usage [WIP]

This section will be written after this tool is (close to) complete...

## Goals

Note that I don't include tasks that I had little to no part in, such as parsing the CUR format,
which is something that the [`ico`](https://docs.rs/ico/0.5.0/ico/) crate already does.

(_though, i did write a CUR parser once, [here](https://github.com/hachispin/rust-cursor-parsing)_)

### Fundamentals

-   [x] Parse the [ANI](<https://en.wikipedia.org/wiki/ANI_(file_format)>) format
-   [x] Write valid Xcursor files

### Quality of life

-   [x] Add upscaling and downscaling for cursors
-   [x] Map cursor names to Xcursor equivalents, similar to [win2xcur-batch](https://github.com/khayalhus/win2xcur-batch)
-   [x] Read [`.inf`](https://en.wikipedia.org/wiki/INF_file) files for smart mappings, like [these](/testing/fixtures/Installer.inf)
-   [x] Generate [`.theme`](https://specifications.freedesktop.org/icon-theme/latest/#file_formats) files

### End goal

-   [x] Produced cursor theme works seamlessly in `/usr/share/icons`
