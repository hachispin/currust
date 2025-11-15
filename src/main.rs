use std::path::Path;

use clap::Parser;
use log;
use miette::Result;
use simplelog::Level;

#[derive(Parser, Debug)]
#[command(version, about, long_about=None)]
struct Args {
    /// Disables logs entirely
    #[arg(short, long)]
    quiet: bool,

    /// Shows all logs with severity ≥ LEVEL. Defaults to "warn"
    #[arg(long, value_name = "LEVEL")]
    log_level: Option<Level>,

    /// Writes logs to FILE instead of the terminal
    #[arg(long, value_name = "FILE")]
    log_file: Option<String>,
}

fn validate_args(args: &Args) -> Result<()> {
    if let Some(v) = &args.log_file {
        Path::canonicalize(v)?;
    }

    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();

    if let Some(v) = &args.log_file {
        println!("totally writing to {v}");
    }

    println!("{args:?}");
    log::warn!("nothing");

    Ok(())
}
