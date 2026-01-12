#![doc = include_str!("../README.md")]
#![warn(
    clippy::pedantic,
    // nursery lints:
    clippy::use_self,
    clippy::or_fun_call,
    clippy::redundant_clone,
    clippy::equatable_if_let,
    clippy::needless_collect,
    // restriction lints:
    clippy::redundant_type_annotations,
    clippy::semicolon_inside_block,
    // annoying one:
    missing_docs
)]

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
