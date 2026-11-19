use std::path::PathBuf;
use std::time::Instant;

use clap::{Parser, Subcommand};
use rand::{RngExt, SeedableRng};

use four2four2::solver::{self, Tables};
use four2four2::twist::{Twist, hsc_stm};
use four2four2::{hsc, state::State};

#[derive(Parser)]
#[command(about = "Two-phase solver for the {4}x{4} 2x2 duoprism (Hyperspeedcube 2)")]
struct Cli {
    /// Where the lookup tables are cached (built on first use, ~415 MB).
    #[arg(long, global = true, default_value = concat!(env!("CARGO_MANIFEST_DIR"), "/tables"))]
    tables: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Solve the scramble of an HSC2 log and write the solution as a new log.
    Solve {
        input: PathBuf,
        output: PathBuf,
        #[command(flatten)]
        search: SearchArgs,
    },
    /// Solve random scrambles. Prints `stm phase1 phase2` per scramble.
    Bench {
        /// Number of scrambles.
        #[arg(short, default_value_t = 100)]
        n: usize,
        #[arg(long, default_value_t = 0)]
        seed: u64,
        #[command(flatten)]
        search: SearchArgs,
    },
}

#[derive(clap::Args)]
struct SearchArgs {
    /// Only target the I/O separation in the η ring, instead of trying all 4
    /// symmetric oll targets (either ring, either axis).
    #[arg(long)]
    no_symmetries: bool,
    /// Try phase 1 solutions up to this many twists longer than optimal.
    #[arg(long, default_value_t = 2, conflicts_with = "greedy")]
    depth: u8,
    /// Take the first optimal phase 1 and don't search further. Cheaper and
    /// worse than `--depth 0`, which tries every optimal phase 1.
    #[arg(long)]
    greedy: bool,
}

impl SearchArgs {
    fn options(&self) -> solver::Options {
        solver::Options {
            symmetries: !self.no_symmetries,
            depth: (!self.greedy).then_some(self.depth),
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let t0 = Instant::now();
    let tables = Tables::load_or_build(&cli.tables);
    eprintln!("tables ready in {:.1?}", t0.elapsed());

    match cli.command {
        Command::Solve {
            input,
            output,
            search,
        } => {
            let text = std::fs::read_to_string(&input)?;
            let scramble = hsc::parse(&text)?.scramble;
            let mut state = State::solved();
            state.apply_all(&scramble);

            let t0 = Instant::now();
            let sol = solver::solve(&tables, &state, search.options());
            let twists = sol.twists();
            eprintln!(
                "stm {} ({} + {}, target {}) in {:.1?}",
                hsc_stm(&twists),
                sol.phase1.len(),
                sol.phase2.len(),
                sol.target,
                t0.elapsed()
            );
            state.apply_all(&twists);
            assert!(state.is_solved(), "solution doesn't solve the scramble");
            std::fs::write(output, hsc::write(&scramble, &twists))?;
        }
        Command::Bench { n, seed, search } => {
            let all = Twist::all();
            let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
            let t0 = Instant::now();
            let mut total = 0;
            for _ in 0..n {
                let mut state = State::solved();
                for _ in 0..1000 {
                    state.apply(all[rng.random_range(0..all.len())]);
                }
                let sol = solver::solve(&tables, &state, search.options());
                state.apply_all(&sol.twists());
                assert!(state.is_solved());
                let stm = hsc_stm(&sol.twists());
                total += stm;
                println!("{stm} {} {}", sol.phase1.len(), sol.phase2.len());
            }
            eprintln!(
                "mean stm {:.2} over {n} scrambles, {:.1?} per solve",
                total as f64 / n as f64,
                t0.elapsed() / n.max(1) as u32
            );
        }
    }
    Ok(())
}
