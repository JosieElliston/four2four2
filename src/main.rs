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
    /// Try phase 1 solutions up to this many twists longer than optimal.
    /// Each step costs about 10x the time.
    #[arg(long, default_value_t = solver::DEFAULT_DEPTH)]
    depth: u8,
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
            let sol = solver::solve(&tables, &state, search.depth);
            let twists = sol.twists();
            eprintln!(
                "stm {}{} ({} + {}, target {}) in {:.1?}",
                hsc_stm(&twists),
                if sol.optimal { " (optimal)" } else { "" },
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
            let mut nodes = 0;
            for _ in 0..n {
                let mut state = State::solved();
                for _ in 0..1000 {
                    state.apply(all[rng.random_range(0..all.len())]);
                }
                let sol = solver::solve(&tables, &state, search.depth);
                state.apply_all(&sol.twists());
                assert!(state.is_solved());
                let stm = hsc_stm(&sol.twists());
                total += stm;
                nodes += sol.nodes;
                println!("{stm} {} {}", sol.phase1.len(), sol.phase2.len());
            }
            eprintln!(
                "mean stm {:.2} over {n} scrambles, {:.1?} per solve, {:.3e} nodes per solve",
                total as f64 / n as f64,
                t0.elapsed() / n.max(1) as u32,
                nodes as f64 / n.max(1) as f64,
            );
        }
    }
    Ok(())
}
