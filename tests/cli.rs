#![cfg(target_os = "linux")]

use std::{
    ffi::CString,
    fs::{self, File},
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    process::Command,
    ptr::NonNull,
    slice,
};

use ico::{IconDir, IconDirEntry, IconImage, ResourceType};
use tempfile::tempdir;
use x11::xcursor::{XcursorFilenameLoadAllImages, XcursorImages, XcursorImagesDestroy};

fn fixture(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("testing/fixtures")
        .join(path)
}

fn currust(input: &Path, out: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_currust"));
    command.arg(input).arg("--out").arg(out);
    command
}

fn run_success(command: &mut Command) {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{command:?} failed with {}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_alias(cursors: &Path, alias: &str, target: &str) {
    let path = cursors.join(alias);
    let link = fs::read_link(&path)
        .unwrap_or_else(|err| panic!("could not read alias {}: {err}", path.display()));
    assert_eq!(link, Path::new(target), "alias {alias}");
    assert!(
        path.is_file(),
        "{} does not resolve to a file",
        path.display()
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Frame {
    size: u32,
    dimensions: (u32, u32),
    hotspot: (u32, u32),
    delay: u32,
}

fn read_frames(path: &Path) -> Vec<Frame> {
    struct LoadedImages(NonNull<XcursorImages>);

    impl Drop for LoadedImages {
        fn drop(&mut self) {
            // SAFETY: This is the handle returned by libXcursor, owned by this guard.
            unsafe { XcursorImagesDestroy(self.0.as_ptr()) };
        }
    }

    let filename = CString::new(path.as_os_str().as_bytes()).unwrap();
    // SAFETY: The C string stays live and null-terminated throughout the call.
    let images = unsafe { XcursorFilenameLoadAllImages(filename.as_ptr()) };
    let images = LoadedImages(
        NonNull::new(images)
            .unwrap_or_else(|| panic!("libXcursor could not load {}", path.display())),
    );

    // SAFETY: The guard keeps the returned list and all its images alive while we copy metadata.
    unsafe {
        let images = images.0.as_ref();
        assert!(images.nimage > 0, "no images in {}", path.display());
        let entries = slice::from_raw_parts(images.images, usize::try_from(images.nimage).unwrap());

        entries
            .iter()
            .map(|&entry| {
                let image = &*entry;
                Frame {
                    size: image.size,
                    dimensions: (image.width, image.height),
                    hotspot: (image.xhot, image.yhot),
                    delay: image.delay,
                }
            })
            .collect()
    }
}

#[test]
fn converts_inf_theme() {
    let out = tempdir().unwrap();
    run_success(&mut currust(&fixture("neuro/Install.inf"), out.path()));

    let theme = out.path().join("Neuro-sama Cursor");
    let index = fs::read_to_string(theme.join("index.theme")).unwrap();
    assert!(index.lines().any(|line| line == "[Icon Theme]"));
    assert!(index.lines().any(|line| line == "Name=Neuro-sama Cursor"));

    let cursors = theme.join("cursors");
    assert_alias(&cursors, "default", "arrow");
    assert_alias(&cursors, "left_ptr_help", "help");
    assert_alias(&cursors, "ns-resize", "top_side");

    assert_eq!(
        read_frames(&cursors.join("left_ptr_help")),
        [Frame {
            size: 32,
            dimensions: (32, 32),
            hotspot: (0, 0),
            delay: 100,
        }; 21]
    );
}

#[test]
fn converts_crs_theme() {
    let out = tempdir().unwrap();
    run_success(&mut currust(
        &fixture("kasane-teto/kasane-teto.crs"),
        out.path(),
    ));

    let theme = out.path().join("placeholder_theme_name");
    assert!(theme.join("index.theme").is_file());

    let cursors = theme.join("cursors");
    assert_alias(&cursors, "default", "arrow");
    assert_alias(&cursors, "not-allowed", "circle");
    assert_alias(&cursors, "pointer", "hand");

    assert_eq!(
        read_frames(&cursors.join("default")),
        [Frame {
            size: 32,
            dimensions: (32, 32),
            hotspot: (0, 0),
            delay: 400,
        }; 2]
    );
    assert_eq!(
        read_frames(&cursors.join("not-allowed")),
        [Frame {
            size: 32,
            dimensions: (32, 32),
            hotspot: (6, 21),
            delay: 0,
        }]
    );
}

fn cursor_entry(size: u32, hotspot: (u16, u16)) -> IconDirEntry {
    let mut image = IconImage::new(size, size);
    image.set_cursor_hotspot(Some(hotspot));
    IconDirEntry::encode(&image).unwrap()
}

#[test]
fn adds_cur_sizes_and_preserves_native_images() {
    let directory = tempdir().unwrap();
    let input = directory.path().join("multi-size.cur");
    let out = directory.path().join("out");
    let mut cursor = IconDir::new(ResourceType::Cursor);
    cursor.add_entry(cursor_entry(64, (7, 11)));
    cursor.add_entry(cursor_entry(32, (3, 5)));
    cursor.write(File::create(&input).unwrap()).unwrap();

    run_success(currust(&input, &out).args([
        "--base-nominal-size",
        "24",
        "--scale-with",
        "nearest",
        "--scale-to",
        "0.5",
        "1.5",
    ]));

    let mut frames = read_frames(&out.join("multi-size"));
    frames.sort_unstable_by_key(|frame| frame.size);
    assert_eq!(
        frames,
        [
            Frame {
                size: 12,
                dimensions: (16, 16),
                hotspot: (1, 2),
                delay: 0,
            },
            Frame {
                size: 24,
                dimensions: (32, 32),
                hotspot: (3, 5),
                delay: 0,
            },
            Frame {
                size: 36,
                dimensions: (48, 48),
                hotspot: (4, 7),
                delay: 0,
            },
            Frame {
                size: 48,
                dimensions: (64, 64),
                hotspot: (7, 11),
                delay: 0,
            },
        ]
    );
}

fn riff_chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut chunk = id.to_vec();
    chunk.extend(u32::try_from(data.len()).unwrap().to_le_bytes());
    chunk.extend_from_slice(data);

    if !data.len().is_multiple_of(2) {
        chunk.push(0);
    }

    chunk
}

fn riff_numbers(id: &[u8; 4], values: &[u32]) -> Vec<u8> {
    let data: Vec<_> = values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    riff_chunk(id, &data)
}

fn animated_cursor(sequence: [u32; 3]) -> Vec<u8> {
    let mut frames = b"fram".to_vec();

    for hotspot in [(1, 2), (3, 4)] {
        let mut cursor = IconDir::new(ResourceType::Cursor);
        cursor.add_entry(cursor_entry(8, hotspot));

        let mut icon = Vec::new();
        cursor.write(&mut icon).unwrap();
        frames.extend(riff_chunk(b"icon", &icon));
    }

    let mut animation = b"ACON".to_vec();
    // The ANI header describes two stored frames and three sequenced playback steps.
    animation.extend(riff_numbers(b"anih", &[36, 2, 3, 0, 0, 0, 0, 6, 3]));
    animation.extend(riff_numbers(b"rate", &[1, 6, 12]));
    animation.extend(riff_numbers(b"seq ", &sequence));
    animation.extend(riff_chunk(b"LIST", &frames));

    riff_chunk(b"RIFF", &animation)
}

#[test]
fn preserves_ani_playback_order_and_step_delays() {
    let directory = tempdir().unwrap();
    let input = directory.path().join("sequenced.ani");
    let out = directory.path().join("out");
    fs::write(&input, animated_cursor([1, 0, 1])).unwrap();

    run_success(&mut currust(&input, &out));

    let frames = read_frames(&out.join("sequenced"));
    let playback: Vec<_> = frames
        .iter()
        .map(|frame| (frame.hotspot, frame.delay))
        .collect();
    assert_eq!(playback, [((3, 4), 17), ((1, 2), 100), ((3, 4), 200)]);
}

#[test]
fn rejects_malformed_ani() {
    let directory = tempdir().unwrap();
    let out = directory.path().join("out");
    let mut truncated = animated_cursor([1, 0, 1]);
    truncated.truncate(truncated.len() - 4);

    for (name, bytes, expected_error) in [
        (
            "bad-sequence",
            animated_cursor([1, 2, 1]),
            "go out of bounds",
        ),
        ("truncated", truncated, "outside of blob"),
    ] {
        let input = directory.path().join(format!("{name}.ani"));
        fs::write(&input, bytes).unwrap();

        let result = currust(&input, &out).output().unwrap();
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(!result.status.success(), "{name} unexpectedly converted");
        assert!(stderr.contains(expected_error), "{name}: {stderr}");
        assert!(!stderr.contains("panicked"), "{name}: {stderr}");
        assert!(!out.join(name).exists());
    }
}
