//! The README's runtime table: solves N random scrambles (the same ones as
//! `bench -n N --seed SEED`) to optimal, one at a time, and reads every
//! `--depth` off the same solves: `--depth d` runs exactly the rounds up to
//! `extra = d` of the optimal search, so its result and time are those after
//! the last round with `extra <= d`.
//!
//! Prints a markdown table, one row per depth until every solve is optimal.
//!
//! usage: depths N [SEED]
use rand::{RngExt, SeedableRng};

use four2four2::solver::{self, Round, Tables};
use four2four2::state::State;
use four2four2::twist::Twist;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let n: usize = args.get(1).expect("usage: depths N [SEED]").parse().unwrap();
    let seed: u64 = args.get(2).map_or(0, |a| a.parse().unwrap());
    let tables = Tables::load_or_build(concat!(env!("CARGO_MANIFEST_DIR"), "/tables").as_ref());
    let all = Twist::all();
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let mut solves: Vec<Vec<Round>> = vec![];
    for i in 0..n {
        let mut state = State::solved();
        for _ in 0..1000 {
            state.apply(all[rng.random_range(0..all.len())]);
        }
        let sol = solver::solve(&tables, &state, None, None).unwrap();
        state.apply_all(&sol.twists());
        assert!(state.is_solved());
        if (i + 1) % 100 == 0 {
            eprintln!("{} of {n}", i + 1);
        }
        solves.push(sol.rounds);
    }
    let max_extra = solves.iter().map(|r| r.last().unwrap().extra).max().unwrap();
    println!("| `--depth` | n | phase 1 STM | phase 2 STM | total STM | range | time |");
    println!("|---|---|---|---|---|---|---|");
    for d in 0..=max_extra {
        let at: Vec<&Round> = solves
            .iter()
            .map(|r| r.iter().rev().find(|x| x.extra <= d).unwrap())
            .collect();
        let mean = |f: &dyn Fn(&Round) -> f64| at.iter().map(|r| f(r)).sum::<f64>() / n as f64;
        let time = mean(&|r| r.time.as_secs_f64());
        let optimal = at.iter().zip(&solves).filter(|(r, s)| r.stm == s.last().unwrap().stm).count();
        println!(
            "| {d} | {n} | {:.2} | {:.2} | {:.2} | {}–{} | {} | ({optimal} of {n} optimal length, {:.3e} nodes)",
            mean(&|r| r.phase1 as f64),
            mean(&|r| r.phase2 as f64),
            mean(&|r| r.stm as f64),
            at.iter().map(|r| r.stm).min().unwrap(),
            at.iter().map(|r| r.stm).max().unwrap(),
            if time < 1.0 {
                format!("{:.3} ms", time * 1e3)
            } else {
                format!("{time:.3} s")
            },
            mean(&|r| r.nodes as f64),
        );
    }
    // how many solves finished at each round
    let mut last = vec![0; max_extra as usize + 1];
    for r in &solves {
        last[r.last().unwrap().extra as usize] += 1;
    }
    println!("last round's extra: {last:?}");
}
