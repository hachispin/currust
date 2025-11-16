//! Contains custom errors, derived from [`thiserror::Error`]

use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

#[derive(Error, Debug, Diagnostic)]
#[error("{error}")]
#[diagnostic(help("{help}"))]
/// Used for displaying diagnostics when parsing CLI arguments.
pub struct ArgParseError {
    error: String,
    #[source_code]
    src: NamedSource<String>,
    #[label("here!")]
    pos: SourceSpan,
    help: String,
}

#[derive(Error, Debug, Diagnostic)]
#[error("{error}")]
#[diagnostic(help("{help}"))]
/// Used as a general error for problems while parsing
/// raw bytes, such as magic bytes not matching.
pub struct BlobError {
    error: String,
    #[source_code]
    src: NamedSource<String>,
    #[label("here!")]
    pos: SourceSpan,
    help: String,
}

impl ArgParseError {
    /// Helper function for computing [`ArgParseError::src`] and [`ArgParseError::pos`].
    /// The first item in the tuple is the `src`, and the second is the `pos`.
    ///
    /// Note: `flag` is [`Option<T>`] to account for positional arguments.
    fn get_src_and_pos(flag: Option<&str>, value: &str) -> (NamedSource<String>, SourceSpan) {
        let src = if let Some(f) = flag {
            format!("{f} {value}")
        } else {
            value.to_string()
        };

        let pos = if let Some(f) = flag {
            (f.len() + 1, value.len())
        } else {
            (0, src.len())
        };

        (NamedSource::new("stdin", src), pos.into())
    }

    /// Used when given a non-existent filepath.
    pub fn missing_file(flag: Option<&str>, value: &str) -> Self {
        let src_pos = Self::get_src_and_pos(flag, value);

        Self {
            error: "file doesn't exist".to_string(),
            src: src_pos.0,
            pos: src_pos.1,
            help: format!("create this file or point to an existing file"),
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

impl BlobError {
    pub fn new(src: &[u8], filename: &str) -> Self {
        let src = format!("{src:?}");
        let src_len = src.len();

        Self {
            error: "something went wrong".to_string(),
            src: NamedSource::new(filename, src),
            pos: (0, src_len).into(),
            help: "yeah you're cooked bro".to_string(),
        }
    }
}
