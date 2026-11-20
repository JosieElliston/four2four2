//! Extends a [`coset`](coset.rs) result by one twist, in slices that can run
//! as separate jobs (eg a Slurm array).
//!
//! DIST is coset's output for T-1: exact up to T-1, upper bounds above. A
//! position whose bound is above T has distance exactly T iff some word of
//! exactly T twists that ends with a twist outside phase 2 lands on it (a
//! shorter seed plus phase 2 twists would already have given it a bound of at
//! most T). So this enumerates the words of length T, like coset's step 1, but
//! records only those open positions it hits. Bounds equal to T are already
//! exact.
//!
//! The word space is split by first three twists; this job takes every
//! NTASKS-th prefix starting at TASK. OUT gets the hit P2 indices as u32 LE,
//! sorted. The union over all tasks is the set of open positions at exactly T.
//!
//! usage: coset_next DIST T TASK NTASKS OUT
use std::collections::HashSet;
use std::sync::Mutex;
use std::time::Instant;

use rayon::prelude::*;

use four2four2::coord::{P1, P2, PHASE1_MOVES, PHASE2_MOVES};
use four2four2::solver::{self, Tables};
use four2four2::state::State;
use four2four2::table::child_distances;

struct Ctx<'a> {
    tables: &'a Tables,
    t: usize,
    allowed: [[bool; 28]; 28],
    phase2: [bool; 28],
    /// Bit per P2 index: bound above T.
    open: &'a [u64],
}

fn dfs(cx: &Ctx, c: P1, d: u8, p2: P2, g: usize, last: usize, hits: &mut Vec<u32>, nodes: &mut u64) {
    *nodes += 1;
    if g == cx.t {
        if d == 0 && !cx.phase2[last] {
            let i = p2.index();
            if cx.open[i / 64] >> (i % 64) & 1 == 1 {
                hits.push(i as u32);
            }
        }
        return;
    }
    let dec = child_distances(d);
    for i in 0..28 {
        if !cx.allowed[last][i] {
            continue;
        }
        let m = &PHASE1_MOVES[i];
        let c2 = c.apply_p1(&m.p1);
        let d2 = dec[cx.tables.p1.get(c2.index()) as usize];
        if g + 1 + d2 as usize <= cx.t {
            dfs(cx, c2, d2, p2.apply(m), g + 1, i, hits, nodes);
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let usage = "usage: coset_next DIST T TASK NTASKS OUT";
    assert_eq!(args.len(), 6, "{usage}");
    let dist = std::fs::read(&args[1]).unwrap();
    let t: usize = args[2].parse().expect(usage);
    let task: usize = args[3].parse().expect(usage);
    let ntasks: usize = args[4].parse().expect(usage);
    let tables = Tables::load_or_build(concat!(env!("CARGO_MANIFEST_DIR"), "/tables").as_ref());

    let mut open = vec![0u64; dist.len().div_ceil(64)];
    for (i, &v) in dist.iter().enumerate() {
        if v as usize > t {
            open[i / 64] |= 1 << (i % 64);
        }
    }
    drop(dist);
    let phase2 = std::array::from_fn(|i| PHASE2_MOVES.iter().any(|m| m.twist == PHASE1_MOVES[i].twist));
    // same pruning as coset: no same-cell pairs, one pair per product
    let mut seen = HashSet::new();
    let mut allowed = [[false; 28]; 28];
    for i in 0..28 {
        for j in 0..28 {
            if PHASE1_MOVES[i].twist.cell() == PHASE1_MOVES[j].twist.cell() {
                continue;
            }
            let mut s = State::solved();
            s.apply(PHASE1_MOVES[i].twist);
            s.apply(PHASE1_MOVES[j].twist);
            allowed[i][j] = seen.insert(s);
        }
    }
    let cx = Ctx { tables: &tables, t, allowed, phase2, open: &open };

    let prefixes: Vec<[usize; 3]> = (0..28)
        .flat_map(|a| (0..28).flat_map(move |b| (0..28).map(move |c| [a, b, c])))
        .filter(|&[a, b, c]| allowed[a][b] && allowed[b][c])
        .enumerate()
        .filter(|(k, _)| k % ntasks == task)
        .map(|(_, p)| p)
        .collect();
    let t0 = Instant::now();
    let all = Mutex::new(Vec::new());
    let nodes = std::sync::atomic::AtomicU64::new(0);
    prefixes.par_iter().for_each(|p| {
        let (mut c, mut p2) = (P1::solved(), P2::solved());
        for &i in p {
            c = c.apply_p1(&PHASE1_MOVES[i].p1);
            p2 = p2.apply(&PHASE1_MOVES[i]);
        }
        let d = solver::p1_distance(&tables, c);
        if 3 + d as usize > t {
            return;
        }
        let (mut hits, mut n) = (vec![], 0);
        dfs(&cx, c, d, p2, 3, p[2], &mut hits, &mut n);
        nodes.fetch_add(n, std::sync::atomic::Ordering::Relaxed);
        all.lock().unwrap().extend(hits);
    });
    let mut hits = all.into_inner().unwrap();
    hits.sort_unstable();
    hits.dedup();
    let bytes: Vec<u8> = hits.iter().flat_map(|i| i.to_le_bytes()).collect();
    std::fs::write(&args[5], bytes).unwrap();
    eprintln!(
        "task {task}/{ntasks}: {} prefixes, {:.3e} nodes, {} hits in {:.1?}",
        prefixes.len(),
        nodes.into_inner() as f64,
        hits.len(),
        t0.elapsed()
    );
}
