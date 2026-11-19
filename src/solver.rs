//! Two-phase solver: scrambled → oll (I on I, O on O, U/D on U/D) → solved.
//! Each phase uses a complete distance table, so each phase is optimal on its
//! own; the total is not.

use std::path::Path;

use crate::coord::{self, P1, P2, PHASE1_MOVES, PHASE2_MOVES, SlotMove};
use crate::geom::Rot;
use crate::state::State;
use crate::table::{self, UNREACHED};
use crate::twist::{Twist, simplify};

pub struct Tables {
    p1: Vec<u8>,
    p2: Vec<u8>,
}

impl Tables {
    /// Loads the tables from `dir`, building and caching any that are missing.
    pub fn load_or_build(dir: &Path) -> Self {
        let p1 = table::load_or_build(&dir.join("phase1.bin"), coord::P1_SIZE, || {
            table::bfs("phase 1", coord::P1_SIZE, P1::solved().index(), |i, f| {
                let c = P1::from_index(i);
                for m in PHASE1_MOVES.iter() {
                    f(c.apply(m).index());
                }
            })
        });
        let p2 = table::load_or_build(&dir.join("phase2.bin"), coord::P2_SIZE, || {
            table::bfs("phase 2", coord::P2_SIZE, P2::solved().index(), |i, f| {
                let c = P2::from_index(i);
                for m in PHASE2_MOVES.iter() {
                    f(c.apply(m).index());
                }
            })
        });
        Tables { p1, p2 }
    }

    pub fn p1(&self) -> &[u8] {
        &self.p1
    }
    pub fn p2(&self) -> &[u8] {
        &self.p2
    }
}

/// Follows the table downhill to distance 0.
fn descend<C: Copy>(
    table: &[u8],
    moves: &[SlotMove],
    mut c: C,
    index: impl Fn(C) -> usize,
    apply: impl Fn(C, &SlotMove) -> C,
) -> Vec<Twist> {
    let mut d = table[index(c)];
    assert_ne!(d, UNREACHED, "position not reachable in this phase");
    let mut out = vec![];
    while d > 0 {
        let (m, next) = moves
            .iter()
            .map(|m| (m, apply(c, m)))
            .find(|&(_, n)| table[index(n)] == d - 1)
            .expect("table has no downhill move");
        out.push(m.twist);
        c = next;
        d -= 1;
    }
    out
}

/// The symmetries that move each of the 4 oll targets onto the one the tables
/// solve: the sorted axis is z (axis 2), w, x, or y respectively.
pub fn target_symmetries() -> [Rot; 4] {
    [
        Rot::IDENT,
        Rot::from_images([(0, false), (1, false), (3, true), (2, false)]), // w -> z
        Rot::from_images([(2, false), (3, false), (0, false), (1, false)]), // x -> z
        Rot::from_images([(3, true), (2, false), (0, false), (1, false)]), // y -> z
    ]
}

#[derive(Clone, Debug)]
pub struct Solution {
    pub phase1: Vec<Twist>,
    pub phase2: Vec<Twist>,
    /// Which of [`target_symmetries`] was used.
    pub target: usize,
}

impl Solution {
    /// Both phases with same-cell twists across the boundary merged.
    pub fn twists(&self) -> Vec<Twist> {
        simplify(&[self.phase1.clone(), self.phase2.clone()].concat())
    }
}

/// Solves via one oll target. Twists are in the frame of `state`.
pub fn solve_with_target(tables: &Tables, state: &State, sym: Rot) -> (Vec<Twist>, Vec<Twist>) {
    // relabel so the target is the z-sorted one, then rotate piece 0 home
    let s = state.conjugated(sym);
    let r = coord::normalizing_rotation(&s);
    let mut s = s.rotated(r);

    let p1 = descend(
        &tables.p1,
        &PHASE1_MOVES,
        P1::from_state(&s),
        P1::index,
        P1::apply,
    );
    s.apply_all(&p1);
    let p2 = descend(
        &tables.p2,
        &PHASE2_MOVES,
        P2::from_state(&s),
        P2::index,
        P2::apply,
    );
    s.apply_all(&p2);
    assert_eq!(
        s,
        State::solved(),
        "phase 2 didn't solve the normalized state"
    );

    // back to the original frame: undo the rotation, then the relabeling
    let back = r.inverse();
    let undo = |ts: Vec<Twist>| -> Vec<Twist> {
        ts.into_iter()
            .map(|t| t.conjugated(back).conjugated(sym.inverse()))
            .collect()
    };
    (undo(p1), undo(p2))
}

/// Tries every oll target and keeps the shortest total.
pub fn solve(tables: &Tables, state: &State) -> Solution {
    solve_with_targets(tables, state, target_symmetries().len())
}

/// Tries the first `n_targets` of [`target_symmetries`] and keeps the
/// shortest total.
pub fn solve_with_targets(tables: &Tables, state: &State, n_targets: usize) -> Solution {
    target_symmetries()
        .into_iter()
        .take(n_targets.max(1))
        .enumerate()
        .map(|(k, sym)| {
            let (phase1, phase2) = solve_with_target(tables, state, sym);
            Solution {
                phase1,
                phase2,
                target: k,
            }
        })
        .min_by_key(|s| s.twists().len())
        .unwrap()
}
