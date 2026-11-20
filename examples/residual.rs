//! Finishes [`coset`](coset.rs): solves the positions whose upper bound in
//! DIST (coset's output) exceeds LIMIT, with the solver bounded at LIMIT. Only
//! one position per orbit is solved, under the 64 symmetries that preserve H:
//! the 16 sign flips, each optionally with the x <-> y swap, then rotating
//! piece 0 back home; and inversion. They all preserve STM.
//!
//! Prints `index stm seconds` per orbit, or `index NONE` when no solution of
//! at most LIMIT exists (a proved antipode candidate).
//!
//! usage: residual DIST LIMIT
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::time::Instant;

use rayon::prelude::*;

use four2four2::coord::{self, P1, P2};
use four2four2::geom::Rot;
use four2four2::solver::{self, Tables};
use four2four2::state::State;
use four2four2::twist::hsc_stm;

/// The position with phase 2 index `i`, built from its phase 2 solution.
fn state_of(tables: &Tables, i: usize) -> State {
    let mut path = vec![];
    solver::p2_distance(tables, P2::from_index(i), usize::MAX, Some(&mut path));
    let mut s = State::solved();
    for t in path.iter().rev() {
        s.apply(t.inverse());
    }
    assert_eq!(P2::from_state(&s).index(), i);
    s
}

/// Index of the inverse position. The P2 coordinate is faithful on H, so
/// that's the inverse permutation.
fn inverse_index(i: usize) -> usize {
    let p = P2::from_index(i).pieces;
    let mut q = [0u8; 16];
    for s in 0..16 {
        q[p[s] as usize] = s as u8;
    }
    P2 { pieces: q }.index()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let usage = "usage: residual DIST LIMIT";
    let dist = std::fs::read(args.get(1).expect(usage)).unwrap();
    let limit: u8 = args.get(2).expect(usage).parse().expect(usage);
    let tables = Tables::load_or_build(concat!(env!("CARGO_MANIFEST_DIR"), "/tables").as_ref());
    let syms: Vec<Rot> = [[0, 1, 2, 3], [1, 0, 2, 3]]
        .into_iter()
        .flat_map(|a| {
            (0..16u8).map(move |neg| {
                Rot::from_images(std::array::from_fn(|i| (a[i], neg >> i & 1 == 1)))
            })
        })
        .collect();

    let over: Vec<usize> = (0..dist.len()).filter(|&i| dist[i] > limit).collect();
    let reps: BTreeSet<usize> = over
        .par_iter()
        .map(|&i| {
            let s = state_of(&tables, i);
            syms.iter()
                .map(|&r| {
                    let c = s.conjugated(r);
                    let c = c.rotated(coord::normalizing_rotation(&c));
                    assert_eq!(P1::from_state(&c), P1::solved(), "symmetry left H");
                    let j = P2::from_state(&c).index();
                    j.min(inverse_index(j))
                })
                .min()
                .unwrap()
        })
        .collect();
    eprintln!(
        "{} positions above {limit}, {} orbits",
        over.len(),
        reps.len()
    );

    let reps: Vec<usize> = reps.into_iter().collect();
    let (done, none) = (AtomicU64::new(0), AtomicU64::new(0));
    let t0 = Instant::now();
    reps.par_iter().for_each(|&i| {
        let s = state_of(&tables, i);
        let t = Instant::now();
        match solver::solve(&tables, &s, None, Some(limit as usize)) {
            Ok(sol) => {
                let twists = sol.twists();
                let mut c = s.clone();
                c.apply_all(&twists);
                assert!(c.is_solved());
                println!("{i} {} {:.2}", hsc_stm(&twists), t.elapsed().as_secs_f64());
            }
            Err(e) => {
                assert!(e.proved);
                none.fetch_add(1, Relaxed);
                println!("{i} NONE");
            }
        }
        let d = done.fetch_add(1, Relaxed) + 1;
        if d % 1000 == 0 {
            eprintln!("{d}/{} in {:.0?}", reps.len(), t0.elapsed());
        }
    });
    eprintln!(
        "{} orbits in {:.1?}; {} need more than {limit}",
        reps.len(),
        t0.elapsed(),
        none.load(Relaxed)
    );
}
