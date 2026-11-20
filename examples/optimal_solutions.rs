//! Counts the optimal solutions of a log's scramble.
//!
//! Full moveset: the optimal STM from the solver, then every twist sequence of
//! that length that solves it, counted in the fixed frame (negative-cell
//! twists only; each also has versions that twist the opposite cell, which
//! differ only by whole-puzzle rotation). Consecutive twists of one cell are
//! never counted, since they'd merge into one.
//!
//! If the position is a phase 2 position for some oll target, also counts its
//! optimal solutions in that target's reduced moveset.
//!
//! usage: optimal_solutions LOG
use std::collections::HashMap;

use four2four2::coord::{self, P1, P2, PHASE1_MOVES, PHASE2_MOVES};
use four2four2::solver::{self, Tables, target_symmetries};
use four2four2::state::State;
use four2four2::table::{self, child_distances};
use four2four2::{hsc, twist::hsc_stm};

struct Count<'a> {
    tables: &'a Tables,
    /// `moves[i][k]`: move `i` seen through target symmetry `k`.
    moves: Vec<[usize; 4]>,
    len: usize,
    start: State,
    path: Vec<usize>,
    found: u64,
    nodes: u64,
}

impl Count<'_> {
    fn dfs(&mut self, c: [P1; 4], d: [u8; 4]) {
        self.nodes += 1;
        let g = self.path.len();
        if g == self.len {
            // every target's phase 1 is solved; check the whole state
            let mut s = self.start.clone();
            for &i in &self.path {
                s.apply(PHASE1_MOVES[i].twist);
            }
            self.found += s.is_solved() as u64;
            return;
        }
        let last = self.path.last().map(|&i| PHASE1_MOVES[i].twist.cell());
        let dec = d.map(child_distances);
        for i in 0..28 {
            if Some(PHASE1_MOVES[i].twist.cell()) == last {
                continue;
            }
            let cc: [P1; 4] = std::array::from_fn(|k| c[k].apply_p1(&PHASE1_MOVES[self.moves[i][k]].p1));
            let dd: [u8; 4] = std::array::from_fn(|k| dec[k][self.tables.p1.get(cc[k].index()) as usize]);
            // solved is in every target
            if g + 1 + *dd.iter().max().unwrap() as usize > self.len {
                continue;
            }
            self.path.push(i);
            self.dfs(cc, dd);
            self.path.pop();
        }
    }
}

fn main() {
    let path = std::env::args().nth(1).expect("usage: optimal_solutions LOG");
    let tables = Tables::load_or_build(concat!(env!("CARGO_MANIFEST_DIR"), "/tables").as_ref());
    let scramble = hsc::parse(&std::fs::read_to_string(&path).unwrap()).unwrap().scramble;
    let mut s = State::solved();
    s.apply_all(&scramble);
    let s = s.rotated(coord::normalizing_rotation(&s));
    let syms = target_symmetries();

    let sol = solver::solve(&tables, &s, None, None).unwrap();
    let len = hsc_stm(&sol.twists());
    let moves = (0..28)
        .map(|i| {
            syms.map(|r| {
                let t = PHASE1_MOVES[i].twist.conjugated(r);
                PHASE1_MOVES.iter().position(|m| m.twist == t).unwrap()
            })
        })
        .collect();
    let c: [P1; 4] = std::array::from_fn(|k| P1::from_state(&s.conjugated(syms[k])));
    let d = c.map(|c| solver::p1_distance(&tables, c));
    let mut count = Count { tables: &tables, moves, len, start: s.clone(), path: vec![], found: 0, nodes: 0 };
    count.dfs(c, d);
    println!("full moveset: optimal {len} STM, {} optimal solutions ({} nodes)", count.found, count.nodes);

    // reduced moveset, for each oll target the position is in
    let mut t2 = None;
    for (k, &r) in syms.iter().enumerate() {
        let sk = s.conjugated(r);
        let sk = sk.rotated(coord::normalizing_rotation(&sk));
        if P1::from_state(&sk) != P1::solved() {
            continue;
        }
        let t2 = t2.get_or_insert_with(|| {
            table::bfs("phase 2", coord::P2_SIZE, P2::solved().index(), |i, f| {
                let c = P2::from_index(i);
                for m in PHASE2_MOVES.iter() {
                    f(c.apply(m).index());
                }
            })
        });
        // geodesics to solved: ways(c) = sum of ways over downhill neighbors
        fn ways(t: &[u8], c: P2, memo: &mut HashMap<usize, u64>) -> u64 {
            let i = c.index();
            if t[i] == 0 {
                return 1;
            }
            if let Some(&w) = memo.get(&i) {
                return w;
            }
            let w = PHASE2_MOVES
                .iter()
                .map(|m| c.apply(m))
                .filter(|n| t[n.index()] + 1 == t[i])
                .map(|n| ways(t, n, memo))
                .sum();
            memo.insert(i, w);
            w
        }
        let p = P2::from_state(&sk);
        let n = ways(t2, p, &mut HashMap::new());
        println!("reduced moveset of oll target {k}: optimal {} STM, {n} optimal solutions", t2[p.index()]);
    }
}
