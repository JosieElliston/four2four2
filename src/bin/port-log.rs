//! Converts a Hyperspeedcube 2.0.0-zeta.12 log of the {4}x{4} 2x2 duoprism, such
//! as one written by the solver, to the format of later Hyperspeedcube versions.

use std::path::PathBuf;

use clap::Parser;

use four2four2::solver::target_symmetries;
use four2four2::twist::Twist;
use four2four2::{hsc, state::State};

#[derive(Parser)]
#[command(
    about = "Port a Hyperspeedcube 2.0.0-zeta.12 log of the {4}x{4} 2x2 duoprism to the latest log format"
)]
struct Cli {
    input: PathBuf,
    output: PathBuf,
    /// Relabel the log by `solver::target_symmetries()[K]`, conjugating every
    /// twist. 3 maps the solver's phase 2 oll target (the one its tables
    /// solve) to the one HSC draws with I as the solid center cell. The log is
    /// rewritten as just its scramble and solution, dropping anything else.
    #[arg(long, value_parser = clap::value_parser!(u8).range(0..4))]
    target: Option<u8>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let mut text = std::fs::read_to_string(&cli.input)?;
    let err = |e: hsc::Error| format!("{}: {e}", cli.input.display());
    if let Some(k) = cli.target {
        let solve = hsc::parse(&text).map_err(err)?;
        let r = target_symmetries()[k as usize];
        let conj = |ts: &[Twist]| -> Vec<Twist> { ts.iter().map(|t| t.conjugated(r)).collect() };
        let (scramble, solution) = (conj(&solve.scramble), conj(&solve.solution));
        let (mut a, mut b) = (State::solved(), State::solved());
        a.apply_all(&solve.scramble);
        b.apply_all(&scramble);
        assert_eq!(a.conjugated(r), b, "relabeling didn't commute with the twists");
        text = hsc::write(&scramble, &solution);
    }
    let ported = hsc::port_to_latest(&text).map_err(err)?;
    std::fs::write(&cli.output, ported)?;
    Ok(())
}
