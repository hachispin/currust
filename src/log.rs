//! Contains [`init_logging`], which uses [`simplelog`] for
//! writing logs to `stdout`/`stderr` or a given log file.

use crate::cli::ParsedArgs;

use std::fs::File;

use log::LevelFilter;
use miette::{IntoDiagnostic, Result};
use simplelog::{self, ConfigBuilder, TermLogger, TerminalMode, WriteLogger};

/// Initializes logging based on the given `args`.
pub fn init_logging(args: &ParsedArgs) -> Result<()> {
    if args.quiet {
        return Ok(());
    }

    // build logging config
    let filter = args.log_level.to_level_filter();
    let config = ConfigBuilder::new()
        .set_target_level(LevelFilter::Off)
        .set_time_level(LevelFilter::Off)
        .build();

    // write to file if specified, else terminal
    if let Some(f) = &args.log_file {
        let stream = File::open(f).into_diagnostic()?;
        WriteLogger::init(filter, config, stream).into_diagnostic()?;
    } else {
        TermLogger::init(
            filter,
            config,
            TerminalMode::Mixed,
            simplelog::ColorChoice::Auto,
        )
        .into_diagnostic()?;
    }

    Ok(())
}
