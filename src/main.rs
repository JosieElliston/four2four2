use std::path::Path;
use std::process::ExitCode;

use four2four2::{hsc, state::State, twist};

const USAGE: &str = "usage:
  four2four2 verify <log.hsc>...        replay logs and check that they end solved
  four2four2 rewrite <in.hsc> <out.hsc> write the effective solve as a minimal log";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("verify") if args.len() >= 2 => verify(&args[1..]),
        Some("rewrite") if args.len() == 3 => rewrite(&args[1], &args[2]),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn load(path: &str) -> Result<hsc::Solve, Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    Ok(hsc::parse(&text).map_err(|e| format!("{path}: {e}"))?)
}

/// Prints one line per log; returns whether all of them end solved.
fn verify(paths: &[String]) -> Result<bool, Box<dyn std::error::Error>> {
    let mut all_ok = true;
    for path in paths {
        let solve = match load(path) {
            Ok(s) => s,
            Err(e) => {
                println!("ERROR   {e}");
                all_ok = false;
                continue;
            }
        };
        let mut state = State::solved();
        state.apply_all(&solve.scramble);
        let scrambled_cells = state.solved_cells();
        state.apply_all(&solve.solution);
        let solved = state.is_solved();
        all_ok &= solved;
        println!(
            "{}  stm {:3}  (simplified {:3})  scramble {:4} twists, {} cells solved  {}",
            if solved { "SOLVED " } else { "UNSOLVED" },
            twist::hsc_stm(&solve.solution),
            twist::simplify(&solve.solution).len(),
            solve.scramble.len(),
            scrambled_cells,
            Path::new(path)
                .file_name()
                .map_or(path.into(), |f| f.to_string_lossy()),
        );
    }
    Ok(all_ok)
}

fn rewrite(input: &str, output: &str) -> Result<bool, Box<dyn std::error::Error>> {
    let solve = load(input)?;
    std::fs::write(output, hsc::write(&solve.scramble, &solve.solution))?;
    Ok(true)
}
