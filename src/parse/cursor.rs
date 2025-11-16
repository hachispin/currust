//! Module for parsing `.cur` files.

use std::{fs, path::Path};

use crate::{errors::BlobError, parse::constants};

use log::debug;
use miette::{ErrReport, IntoDiagnostic, Result};

pub fn parse_cur(fp: &Path) -> Result<()> {
    debug!("Parsing windows cursor at {}", fp.to_string_lossy());

    let bytes = fs::read(fp).into_diagnostic()?;

    if &bytes[0..4] != constants::CUR_MAGIC {
        return Err(ErrReport::from(BlobError::new(
            &bytes[0..4],
            &fp.to_string_lossy(),
        )));
    }

    Ok(())
}
