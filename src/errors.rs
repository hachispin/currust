//! Contains custom errors, derived from [`thiserror::Error`]

use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

#[derive(Error, Debug, Diagnostic)]
#[error("{error}")]
#[diagnostic(help("{help}"))]
/// Used for displaying diagnostics when parsing CLI arguments.
pub struct ArgError {
    error: String,
    #[source_code]
    src: String,
    #[label("here!")]
    pos: SourceSpan,
    help: String,
}

impl ArgError {
    /// Helper function for computing [`ArgError::pos`] and generating [`ArgError::src`].
    ///
    /// Note: `flag` is [`Option<T>`] to account for positional arguments.
    fn get_src_and_pos(flag: Option<&str>, value: &str) -> (String, SourceSpan) {
        let src = if let Some(f) = flag {
            format!("flags: {f}\nvalue: {value}")
        } else {
            value.to_string()
        };

        let pos = if let Some(f) = flag {
            (f.len() + 15, value.len()) // +15 = len("flags: ") + len("\nvalue: ")
        } else {
            (0, src.len())
        };

        (src, pos.into())
    }

    /// Used when given a non-existent filepath.
    pub fn path_doesnt_exist(flag: Option<&str>, value: &str) -> Self {
        let src_pos = Self::get_src_and_pos(flag, value);

        Self {
            error: "path doesn't exist".to_string(),
            src: src_pos.0,
            pos: src_pos.1,
            help: format!("create this path or point to an existing path"),
        }
    }

    /// Used when receiving a file with the wrong/no file extension
    pub fn invalid_file_ext(
        flag: Option<&str>,
        value: &str,
        received_ext: Option<&str>,
        expected_ext: &str,
    ) -> Self {
        let src_pos = Self::get_src_and_pos(flag, value);

        let error = if let Some(ext) = received_ext {
            format!("expected file extension '.{expected_ext}', instead got '.{ext}'")
        } else {
            format!("expected file extension '.{expected_ext}', instead found no extension")
        };

        Self {
            error,
            src: src_pos.0,
            pos: src_pos.1,
            help: format!("point to a file with the expected '.{expected_ext}' extension"),
        }
    }
}
