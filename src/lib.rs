#![doc = include_str!("../README.md")]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]

pub mod cli;
pub mod cursors;
pub mod scaling;

/// The project root for tests.
///
/// This does not include a trailing slash.
#[macro_export]
macro_rules! root {
    () => {
        env!("CARGO_MANIFEST_DIR")
    };
}
