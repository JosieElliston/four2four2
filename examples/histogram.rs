//! Optimal STM of random positions, for the empirical full puzzle histogram in
//! antipode.md: solves the scrambles of `bench -n N --seed SEED` in parallel
//! (all cores, or `RAYON_NUM_THREADS`), then prints `stm count` per length.
//!
//! usage: histogram N SEED
use std::collections::BTreeMap;

use rand::{RngExt, SeedableRng};
use rayon::prelude::*;

use four2four2::solver::{self, Tables};
use four2four2::state::State;
use four2four2::twist::{Twist, hsc_stm};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let usage = "usage: histogram N SEED";
    let n: usize = args.get(1).expect(usage).parse().expect(usage);
    let seed: u64 = args.get(2).expect(usage).parse().expect(usage);
    let tables = Tables::load_or_build(concat!(env!("CARGO_MANIFEST_DIR"), "/tables").as_ref());
    let all = Twist::all();
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let states: Vec<State> = (0..n)
        .map(|_| {
            let mut s = State::solved();
            for _ in 0..1000 {
                s.apply(all[rng.random_range(0..all.len())]);
            }
            s
        })
        .collect();
    let stms: Vec<usize> = states
        .par_iter()
        .map(|s| {
            let sol = solver::solve(&tables, s, None, None).unwrap();
            let mut check = s.clone();
            check.apply_all(&sol.twists());
            assert!(check.is_solved());
            hsc_stm(&sol.twists())
        })
        .collect();
    let mut counts = BTreeMap::<usize, u64>::new();
    for &stm in &stms {
        *counts.entry(stm).or_default() += 1;
    }
    for (stm, count) in counts {
        println!("{stm} {count}");
    }
}
