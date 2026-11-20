//! Converts a Hyperspeedcube 2.0.0-zeta.12 log of the {4}x{4} 2x2 duoprism, such
//! as one written by the solver, to the format of later Hyperspeedcube versions.

use std::path::PathBuf;

use clap::Parser;

use four2four2::hsc;

#[derive(Parser)]
#[command(
    about = "Port a Hyperspeedcube 2.0.0-zeta.12 log of the {4}x{4} 2x2 duoprism to the latest log format"
)]
struct Cli {
    input: PathBuf,
    output: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let text = std::fs::read_to_string(&cli.input)?;
    let ported = hsc::port_to_latest(&text).map_err(|e| format!("{}: {e}", cli.input.display()))?;
    std::fs::write(&cli.output, ported)?;
    Ok(())
}
