use std::path::Path;
use std::process::ExitCode;

use std::time::Instant;

use four2four2::solver::{self, Tables};
use four2four2::{hsc, state::State, twist};

const USAGE: &str = "usage:
  four2four2 verify <log.hsc>...        replay logs and check that they end solved
  four2four2 rewrite <in.hsc> <out.hsc> write the effective solve as a minimal log
  four2four2 solve <in.hsc> <out.hsc>   solve a log's scramble and write the solution as a log
  four2four2 bench [n]                  solve n random 1000-twist scrambles (default 10)

tables are built on first use (a few hundred MB) and cached in tables/";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("verify") if args.len() >= 2 => verify(&args[1..]),
        Some("rewrite") if args.len() == 3 => rewrite(&args[1], &args[2]),
        Some("solve") if args.len() == 3 => solve(&args[1], &args[2]),
        Some("bench") if args.len() <= 2 => match args.get(1).map_or(Ok(10), |n| n.parse()) {
            Ok(n) => bench(n),
            Err(e) => Err(e.into()),
        },
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

fn tables() -> Tables {
    let t0 = Instant::now();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tables");
    let t = Tables::load_or_build(&dir);
    eprintln!("tables ready in {:.1?}", t0.elapsed());
    t
}

fn report(sol: &solver::Solution, elapsed: std::time::Duration) -> String {
    format!(
        "stm {:3} = {:2} + {:2}{}  target {}  {:.1?}",
        twist::hsc_stm(&sol.twists()),
        sol.phase1.len(),
        sol.phase2.len(),
        if sol.twists().len() < sol.phase1.len() + sol.phase2.len() {
            " (merged)"
        } else {
            ""
        },
        sol.target,
        elapsed,
    )
}

fn solve(input: &str, output: &str) -> Result<bool, Box<dyn std::error::Error>> {
    let scramble = load(input)?.scramble;
    let tables = tables();
    let mut state = State::solved();
    state.apply_all(&scramble);

    let t0 = Instant::now();
    let sol = solver::solve(&tables, &state);
    println!("{}", report(&sol, t0.elapsed()));

    let twists = sol.twists();
    state.apply_all(&twists);
    assert!(state.is_solved(), "solution doesn't solve the scramble");
    std::fs::write(output, hsc::write(&scramble, &twists))?;
    Ok(true)
}

fn bench(n: usize) -> Result<bool, Box<dyn std::error::Error>> {
    let tables = tables();
    let all = twist::Twist::all();
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut total = 0;
    for _ in 0..n {
        let mut state = State::solved();
        for _ in 0..1000 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            state.apply(all[(x % all.len() as u64) as usize]);
        }
        let t0 = Instant::now();
        let sol = solver::solve(&tables, &state);
        let elapsed = t0.elapsed();
        state.apply_all(&sol.twists());
        assert!(state.is_solved());
        total += twist::hsc_stm(&sol.twists());
        println!("{}", report(&sol, elapsed));
    }
    println!("mean stm {:.2}", total as f64 / n as f64);
    Ok(true)
}
