//! Exact full-moveset distances over H, the phase 2 positions (oll solved),
//! up to `T` twists, by the coset method:
//!
//! 1. DFS over the 28 fixed-frame twists from solved, up to T twists, pruned by
//!    the phase 1 table. Each word that lands in H and ends with a twist
//!    outside the 12 phase 2 twists is a seed; keep the shortest per position.
//! 2. BFS over H with the 12 phase 2 twists, starting from the seeds.
//!
//! Any word landing in H is a seed followed by phase 2 twists, so the result
//! is exact up to T. Above T it's an upper bound from a real solution.
//!
//! usage: coset T [OUT]. OUT gets one byte per P2 index.
use std::collections::HashSet;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering::Relaxed};
use std::time::Instant;

use rayon::prelude::*;

use four2four2::coord::{P1, P2, P2_SIZE, PHASE1_MOVES, PHASE2_MOVES};
use four2four2::solver::{self, Tables};
use four2four2::state::State;
use four2four2::table::child_distances;

struct Ctx<'a> {
    tables: &'a Tables,
    t: usize,
    /// `allowed[i][j]`: move `j` may follow move `i`.
    allowed: [[bool; 28]; 28],
    /// Which moves are phase 2 twists.
    phase2: [bool; 28],
    seed: &'a [AtomicU8],
}

fn dfs(cx: &Ctx, c: P1, d: u8, p2: P2, g: usize, last: usize, nodes: &mut u64) {
    *nodes += 1;
    if d == 0 && !cx.phase2[last] {
        cx.seed[p2.index()].fetch_min(g as u8, Relaxed);
    }
    if g == cx.t {
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
            dfs(cx, c2, d2, p2.apply(m), g + 1, i, nodes);
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let t: usize = args[1].parse().expect("usage: coset T [OUT]");
    let tables = Tables::load_or_build(concat!(env!("CARGO_MANIFEST_DIR"), "/tables").as_ref());
    let phase2 = std::array::from_fn(|i| {
        PHASE2_MOVES
            .iter()
            .any(|m| m.twist == PHASE1_MOVES[i].twist)
    });
    // No same-cell pairs, and of each set of pairs with equal products, only
    // the first. The shortlex-least word of each position avoids both.
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

    let seed: Vec<AtomicU8> = (0..P2_SIZE).map(|_| AtomicU8::new(u8::MAX)).collect();
    seed[P2::solved().index()].store(0, Relaxed);
    let cx = Ctx {
        tables: &tables,
        t,
        allowed,
        phase2,
        seed: &seed,
    };
    let t0 = Instant::now();
    let nodes = AtomicU64::new(0);
    // one task per first move, split again by second move for balance
    let firsts: Vec<(usize, usize)> = (0..28)
        .flat_map(|i| (0..28).map(move |j| (i, j)))
        .filter(|&(i, j)| allowed[i][j])
        .collect();
    let solved = P1::solved();
    for (i, m) in PHASE1_MOVES.iter().enumerate() {
        if t >= 1 && solved.apply_p1(&m.p1) == solved && !phase2[i] {
            seed[P2::solved().apply(m).index()].fetch_min(1, Relaxed);
        }
    }
    if t >= 2 {
        firsts.par_iter().for_each(|&(i, j)| {
            let (mi, mj) = (&PHASE1_MOVES[i], &PHASE1_MOVES[j]);
            let c = solved.apply_p1(&mi.p1).apply_p1(&mj.p1);
            let d = solver::p1_distance(&tables, c);
            if 2 + d as usize <= t {
                let mut n = 0;
                dfs(&cx, c, d, P2::solved().apply(mi).apply(mj), 2, j, &mut n);
                nodes.fetch_add(n, Relaxed);
            }
        });
    }
    let mut hist = vec![0u64; t + 1];
    for s in &seed {
        let v = s.load(Relaxed);
        if v != u8::MAX {
            hist[v as usize] += 1;
        }
    }
    eprintln!(
        "T={t}: {:.3e} nodes in {:.1?}; seeds by length {hist:?}",
        nodes.load(Relaxed) as f64,
        t0.elapsed()
    );

    let t0 = Instant::now();
    let dist = seed;
    for g in 0.. {
        let changed = AtomicU64::new(0);
        (0..P2_SIZE).into_par_iter().for_each(|i| {
            if dist[i].load(Relaxed) != g {
                return;
            }
            let c = P2::from_index(i);
            for m in PHASE2_MOVES.iter() {
                if dist[c.apply(m).index()].fetch_min(g + 1, Relaxed) > g + 1 {
                    changed.fetch_add(1, Relaxed);
                }
            }
        });
        // seeds can still appear up to T even with no change
        if changed.load(Relaxed) == 0 && g as usize >= t {
            break;
        }
    }
    let mut hist = vec![0u64; 64];
    for s in &dist {
        hist[s.load(Relaxed) as usize] += 1;
    }
    hist.truncate(hist.iter().rposition(|&x| x > 0).unwrap() + 1);
    eprintln!(
        "bfs in {:.1?}; positions by distance (exact up to {t}, upper bounds above): {hist:?}",
        t0.elapsed()
    );
    if let Some(out) = args.get(2) {
        let bytes: Vec<u8> = dist.iter().map(|s| s.load(Relaxed)).collect();
        std::fs::write(out, bytes).unwrap();
    }
}
