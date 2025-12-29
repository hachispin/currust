# currust

A **work in progress** tool written in Rust to convert cursors between Windows and Linux 
(specifically, the [Xcursor](http://justsolve.archiveteam.org/wiki/Xcursor) format)

Unlike currently existing tools such as [win2xcur](https://github.com/quantum5/win2xcur), which
require a substantial amount of manual work, this project aims to be as **easy to use** as possible.

## Goals

- [x] Modify [`.cur`](https://en.wikipedia.org/wiki/ICO_(file_format)) files to valid Xcursor
- [ ] Ditto, but for the [`.ani`](https://en.wikipedia.org/wiki/ANI_(file_format)) format
- [x] Add upscaling and downscaling for cursors
- [ ] Map cursor names to Xcursor equivalents, similar to [win2xcur-batch](https://github.com/khayalhus/win2xcur-batch)
- [ ] Read [`.inf`](https://en.wikipedia.org/wiki/INF_file) files for smart mappings, like [these](/testing/data/[The%20Herta%20Cursor%20ver.2.0.0]/Installer.inf).
- [ ] Generate [`.theme`](https://specifications.freedesktop.org/icon-theme/latest/#file_formats) files
- [ ] Produced theme directory works when placed in `/usr/share/icons`
