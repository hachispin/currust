#![doc=include_str!("../README.md")]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]
#![warn(rust_2018_idioms)]

macro_rules! throw {
    ($e:expr) => {
        return Err(ErrReport::from($e))
    };
}

pub mod cli;
pub mod cursors;
pub mod errors;
pub mod logging;
