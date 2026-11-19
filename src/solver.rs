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

/// Default for how many twists past optimal phase 1 to search.
pub const DEFAULT_DEPTH: u8 = 2;

/// The frame a target is solved in: relabel by `sym` so the target is the
/// z-sorted one, then rotate by `rot` so piece 0 is home.
struct Frame {
    sym: Rot,
    rot: Rot,
}

impl Frame {
    fn new(state: &State, sym: Rot) -> (Self, State) {
        let s = state.conjugated(sym);
        let rot = coord::normalizing_rotation(&s);
        (Frame { sym, rot }, s.rotated(rot))
    }

    /// Maps twists in this frame back to the original one.
    fn to_original(&self, ts: &[Twist]) -> Vec<Twist> {
        let (back, unsym) = (self.rot.inverse(), self.sym.inverse());
        ts.iter()
            .map(|t| t.conjugated(back).conjugated(unsym))
            .collect()
    }
}

/// Best phase 1 found so far, in a normalized frame.
struct Best {
    total: usize,
    target: usize,
    phase1: Vec<Twist>,
    p2: P2,
}

/// Depth-first search over phase 1 paths of exactly `remaining` more twists,
/// pruned by the exact phase 1 table. At the end of each path, the phase 2
/// table gives the rest of the length.
#[allow(clippy::too_many_arguments)]
fn search_phase1(
    tables: &Tables,
    c1: P1,
    c2: P2,
    remaining: u8,
    path: &mut Vec<Twist>,
    target: usize,
    best: &mut Option<Best>,
) {
    if remaining == 0 {
        let d2 = tables.p2[c2.index()];
        debug_assert_ne!(d2, UNREACHED);
        let total = path.len() + d2 as usize;
        if best.as_ref().is_none_or(|b| total < b.total) {
            *best = Some(Best {
                total,
                target,
                phase1: path.clone(),
                p2: c2,
            });
        }
        return;
    }
    for m in PHASE1_MOVES.iter() {
        // two twists of one cell are one twist, which is tried on its own
        if path.last().is_some_and(|t| t.cell() == m.twist.cell()) {
            continue;
        }
        let n1 = c1.apply(m);
        if tables.p1[n1.index()] > remaining - 1 {
            continue;
        }
        path.push(m.twist);
        search_phase1(tables, n1, c2.apply(m), remaining - 1, path, target, best);
        path.pop();
    }
}

/// Solves `state`, trying every phase 1 up to `depth` twists longer than
/// optimal for each oll target, and keeping the shortest total. Twists in the
/// result are in the frame of `state`.
pub fn solve(tables: &Tables, state: &State, depth: u8) -> Solution {
    let frames: Vec<(Frame, State)> = target_symmetries()
        .into_iter()
        .map(|sym| Frame::new(state, sym))
        .collect();

    let starts: Vec<(P1, P2, u8)> = frames
        .iter()
        .map(|(_, s)| {
            let c1 = P1::from_state(s);
            (c1, P2::from_state(s), tables.p1[c1.index()])
        })
        .collect();

    let mut best: Option<Best> = None;
    // every phase 1 of each length, shortest lengths first across all targets
    // so a good total found on one target cuts off long searches on the rest
    for extra in 0..=depth {
        for (target, &(c1, c2, d1)) in starts.iter().enumerate() {
            let len = d1 + extra;
            // phase 2 can't be negative, so this length can't win
            if best.as_ref().is_some_and(|b| len as usize >= b.total) {
                continue;
            }
            let mut path = vec![];
            search_phase1(tables, c1, c2, len, &mut path, target, &mut best);
        }
    }

    let best = best.expect("no solution found");
    let phase2 = descend(&tables.p2, &PHASE2_MOVES, best.p2, P2::index, P2::apply);
    let (frame, s) = &frames[best.target];
    let mut check = s.clone();
    check.apply_all(&best.phase1);
    check.apply_all(&phase2);
    assert_eq!(check, State::solved(), "didn't solve the normalized state");
    Solution {
        phase1: frame.to_original(&best.phase1),
        phase2: frame.to_original(&phase2),
        target: best.target,
    }
}
