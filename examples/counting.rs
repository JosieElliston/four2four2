//! Combinatorial lower bounds on god's numbers: the least n such that the
//! number of twist sequences of length <= n is at least the number of
//! positions. Sequences skip a twist on the same cell as the previous one, and
//! of each set of two-twist sequences with equal products, keep only the first.
use std::collections::HashSet;

use four2four2::coord::{P1_SIZE, P2_SIZE, PHASE1_MOVES, PHASE2_MOVES, SlotMove};
use four2four2::state::State;

/// `allowed[i][j]`: whether `moves[j]` may follow `moves[i]`.
fn allowed(moves: &[SlotMove]) -> Vec<Vec<bool>> {
    let mut seen = HashSet::new();
    let n = moves.len();
    let mut a = vec![vec![false; n]; n];
    for i in 0..n {
        for j in 0..n {
            if moves[i].twist.cell() == moves[j].twist.cell() {
                continue;
            }
            let mut s = State::solved();
            s.apply(moves[i].twist);
            s.apply(moves[j].twist);
            a[i][j] = seen.insert(s);
        }
    }
    a
}

fn bound(name: &str, moves: &[SlotMove], positions: f64) {
    let a = allowed(moves);
    let n = moves.len();
    // ends[j]: sequences of the current length ending in move j
    let mut ends = vec![1.0f64; n];
    let mut total = 1.0 + n as f64;
    let mut len = 1;
    while total < positions {
        ends = (0..n)
            .map(|j| (0..n).filter(|&i| a[i][j]).map(|i| ends[i]).sum())
            .collect();
        total += ends.iter().sum::<f64>();
        len += 1;
    }
    println!("{name}: {positions:.4e} positions, {total:.4e} sequences of length <= {len}, so at least {len}");
}

fn main() {
    let fact = |n: u32| (1..=n).map(f64::from).product::<f64>();
    bound("phase 1", &PHASE1_MOVES, P1_SIZE as f64);
    bound("phase 2 (reduced)", &PHASE2_MOVES, P2_SIZE as f64);
    bound("phase 2 (full)", &PHASE1_MOVES, P2_SIZE as f64);
    bound("whole puzzle", &PHASE1_MOVES, fact(15) * 2f64.powi(13));
}
