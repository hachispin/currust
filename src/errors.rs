//! Contains custom errors, derived from [`thiserror::Error`]

use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

#[derive(Error, Debug, Diagnostic)]
#[error("{error}")]
#[diagnostic(help("{help}"))]
pub struct ArgParseError {
    error: String,
    #[source_code]
    src: NamedSource<String>,
    #[label("here!")]
    pos: SourceSpan,
    help: String,
}

impl ArgParseError {
    /// Creates a diagnostic given an invalid filepath arggument.
    ///
    /// Note: `flag` is [`Option<T>`] to account for positional arguments.
    pub fn invalid_file(flag: Option<&str>, value: &str) -> Self {
        let src = if let Some(f) = flag {
            format!("{f} {value}")
        } else {
            value.to_string()
        };

        // highlight value only
        let pos = if let Some(f) = flag {
            (f.len() + 1, value.len())
        } else {
            (0, src.len())
        };

        Self {
            error: "file doesn't exist".to_string(),
            src: NamedSource::new("stdin", src),
            pos: pos.into(),
            help: format!("create this file or point to an existing file"),
        }
    }
}
