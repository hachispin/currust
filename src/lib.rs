#![doc=include_str!("../README.md")]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]
#![warn(rust_2018_idioms)]

// Used in `project_root!()` macro.
#[allow(unused_imports)]
use std::env;

/// Shorthand for returning an error (for [`miette::Result`])
macro_rules! throw {
    ($e:expr) => {
        return Err(ErrReport::from($e))
    };
}

/// The project root.
///
/// This is only used for tests and shouldn't be used anywhere
/// else, since this env var doesn't exist in binaries.
/// 
/// Also, this is a macro and not a `const &str` for
/// compile-time concatenation (with `concat!()`).
#[allow(unused_macros)]
macro_rules! project_root {
    () => {
        env!("CARGO_MANIFEST_DIR")
    };
}

pub mod cli;
pub mod cursors;
pub mod errors;
pub mod logging;
