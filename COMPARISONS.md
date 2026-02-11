# Comparisons

This comparison is between [currust] \(this project), [win2xcur], [ani-to-xcursor] and [ani2xcursor].
If any new tools get released (or popularized), I'll try to add them here if it introduces something new.

## Features

As far as I'm aware, no other tool other than [currust] allows scaling of cursors _along with a choice
of algorithm_. [win2xcur] and [ani2xcursor] have scaling, but the algorithm they use can't be specified.

As for features not present in this project: [win2xcur] has bi-directional conversions (i.e, from Xcursor to ANI)
and [ani2xcursor] has a "preview" feature, as well as being able to use manifests to customize cursor roles.

## Performance

### Conversion

Only theme conversion.

| Tool used        | Language | Time (ms)   | Ratio        |
| ---------------- | -------- | ----------- | ------------ |
| [currust]        | Rust     | 4.0 ± 0.2   | 1            |
| [ani-to-xcursor] | Rust     | 9.4 ± 1.1   | 2.32 ± 0.29  |
| [ani2xcursor]    | C++      | 15.7 ± 0.8  | 3.88 ± 0.24  |
| [win2xcur]       | Python   | 285.2 ± 2.3 | 70.53 ± 2.70 |

### Conversion, with scaling

Theme conversion, along with scaling from 32x32 to 256x256 (8x scale factor) using bilinear interpolation.

| Tool used     | Language | Time (ms)  | Ratio        |
| ------------- | -------- | ---------- | ------------ |
| [currust]     | Rust     | 52.1 ± 2.8 | 1            |
| [ani2xcursor] | C++      | 2975 ± 26  | 57.04 ± 3.10 |

### Notes

In both scenarios, [currust] is the fastest by a pretty large margin. You can see the full details [here](https://gist.github.com/hachispin/33cdae0ecd3542f1836d9164a307a7c2).

## Dependencies

Uniquely, this project doesn't have any runtime dependencies:

- [currust] only uses Rust crates, binary works independently (though probably larger)
- [win2xcur] uses [`Wand`](https://docs.wand-py.org/en/0.6.12/), which uses the MagickWand library
- [ani-to-xcursor] shells out to `xcursorgen`
- [ani2xcursor] dynamically links into `libXcursor`

[currust]: ../../
[win2xcur]: https://github.com/quantum5/win2xcur
[ani-to-xcursor]: https://github.com/nicdgonzalez/ani-to-xcursor
[ani2xcursor]: https://github.com/yuzujr/ani2xcursor
