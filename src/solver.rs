//! Two-phase solver: scrambled → oll (I on I, O on O, U/D on U/D) → solved.
//! Each phase uses a complete distance table, so each phase is optimal on its
//! own; the total is not.

use std::path::Path;
use std::sync::LazyLock;

use crate::coord::{self, P1, P1Move, P2, PHASE1_MOVES, PHASE2_MOVES, SlotMove};
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

/// Symmetries that fix slot 0 (so they keep piece 0 home) and move each of
/// the 4 oll targets onto the one the tables solve: the sorted axis is z
/// (axis 2), w, x, or y respectively. They only permute axes, so they map
/// negative cells to negative cells, and the twists the tables use to
/// themselves. The w one is a mirror, which is fine for relabeling.
pub fn target_symmetries() -> [Rot; 4] {
    let perm = |a: [u8; 4]| Rot::from_images(a.map(|x| (x, false)));
    [
        Rot::IDENT,
        perm([0, 1, 3, 2]), // z <-> w
        perm([2, 3, 0, 1]), // x <-> z, y <-> w
        perm([3, 2, 1, 0]), // x <-> w, y <-> z
    ]
}

/// Precomputed effect of each target symmetry on slots and on phase 1 moves.
struct Frames {
    syms: [Rot; 4],
    /// `slot[k][p]`: where symmetry `k` sends slot `p`.
    slot: [[u8; 16]; 4],
    /// `moves[i][k]`: move `i` of [`PHASE1_MOVES`] seen through symmetry `k`.
    moves: [[P1Move; 4]; 28],
}

static FRAMES: LazyLock<Frames> = LazyLock::new(|| {
    let syms = target_symmetries();
    let slot =
        syms.map(|r| std::array::from_fn(|p| coord::slot_of(r.apply(coord::coords_of(p))) as u8));
    let moves = std::array::from_fn(|i| {
        syms.map(|r| {
            let t = PHASE1_MOVES[i].twist.conjugated(r);
            PHASE1_MOVES.iter().find(|n| n.twist == t).unwrap().p1
        })
    });
    Frames { syms, slot, moves }
});

#[derive(Clone, Debug)]
pub struct Solution {
    pub phase1: Vec<Twist>,
    pub phase2: Vec<Twist>,
    /// Which of [`target_symmetries`] was used.
    pub target: usize,
    /// Search nodes visited.
    pub nodes: u64,
    /// Whether the search proved no shorter solution exists.
    pub optimal: bool,
}

impl Solution {
    /// Both phases with same-cell twists across the boundary merged.
    pub fn twists(&self) -> Vec<Twist> {
        simplify(&[self.phase1.clone(), self.phase2.clone()].concat())
    }
}

/// Default for how many twists past optimal phase 1 to search.
pub const DEFAULT_DEPTH: u8 = 2;

/// Best phase 1 found so far, in the normalized frame.
struct Best {
    total: usize,
    target: usize,
    phase1: Vec<Twist>,
    /// Phase 2 position, seen through the target's symmetry.
    p2: P2,
}

struct Search<'a> {
    tables: &'a Tables,
    /// Phase 1 lengths tried this round, per target.
    budget: [u8; 4],
    best: Option<Best>,
    path: Vec<Twist>,
    nodes: u64,
}

impl Search<'_> {
    fn bound(&self) -> usize {
        self.best.as_ref().map_or(usize::MAX, |b| b.total)
    }

    /// A finished phase 1 for target `k`: finish with phase 2.
    #[cfg_attr(feature = "profiling", inline(never))]
    fn leaf(&mut self, k: usize, p2: P2) {
        let slot = &FRAMES.slot[k];
        let mut pieces = [0; 16];
        for p in 0..16 {
            pieces[slot[p] as usize] = slot[p2.pieces[p] as usize];
        }
        let p2k = P2 { pieces };
        let total = self.path.len() + self.tables.p2[p2k.index()] as usize;
        if total < self.bound() {
            self.best = Some(Best {
                total,
                target: k,
                phase1: self.path.clone(),
                p2: p2k,
            });
        }
    }

    /// Every child's phase 1 coordinates and distances, for children not on
    /// `skip` (a range of [`PHASE1_MOVES`]). The search is bound by these
    /// loads, so they're issued with no branches on them, letting them overlap.
    #[cfg_attr(feature = "profiling", inline(never))]
    fn expand(&self, c: &[P1; 4], skip: std::ops::Range<usize>) -> ([[P1; 4]; 28], [[u8; 4]; 28]) {
        let moves = &FRAMES.moves;
        let p1 = &self.tables.p1[..];
        let mut cc = [[P1::solved(); 4]; 28];
        let mut dd = [[0u8; 4]; 28];
        let kids = (0..28).filter(|i| !skip.contains(i));
        for i in kids.clone() {
            for k in 0..4 {
                cc[i][k] = c[k].apply_p1(&moves[i][k]);
            }
        }
        for i in kids {
            for k in 0..4 {
                dd[i][k] = p1[cc[i][k].index()];
            }
        }
        (cc, dd)
    }

    /// `c[k]` is the phase 1 coordinate seen through target symmetry `k`,
    /// `d[k]` its distance. `p2` holds the pieces in the normalized frame.
    fn dfs(&mut self, c: [P1; 4], d: [u8; 4], p2: P2) {
        self.nodes += 1;
        let g = self.path.len();
        for (k, &dk) in d.iter().enumerate() {
            if dk == 0 && g == self.budget[k] as usize {
                self.leaf(k, p2);
            }
        }
        // a child must stay within some target's budget
        if self.budget.iter().all(|&b| g >= b as usize) {
            return;
        }
        // Two twists of one cell are one twist, which is tried on its own, so
        // skip the last twist's cell. PHASE1_MOVES is grouped by cell, 7 each.
        let skip = self.path.last().map_or(0..0, |t| {
            let s = PHASE1_MOVES
                .iter()
                .position(|m| m.twist.cell() == t.cell())
                .unwrap();
            s..s + 7
        });
        let (cc, dd) = self.expand(&c, skip.clone());
        for (i, m) in PHASE1_MOVES.iter().enumerate() {
            if skip.contains(&i) {
                continue;
            }
            // solved is in every target, so reaching it takes at least the
            // largest phase 1 distance; and some target must still be
            // reachable within its phase 1 budget.
            let worst = *dd[i].iter().max().unwrap() as usize;
            if g + 1 + worst >= self.bound() {
                continue;
            }
            if !(0..4).any(|k| g + 1 + dd[i][k] as usize <= self.budget[k] as usize) {
                continue;
            }
            self.path.push(m.twist);
            self.dfs(cc[i], dd[i], p2.apply(m));
            self.path.pop();
        }
    }
}

/// Solves `state`, trying every phase 1 up to `depth` twists longer than
/// optimal for each oll target, and keeping the shortest total. Twists in the
/// result are in the frame of `state`.
pub fn solve(tables: &Tables, state: &State, depth: u8) -> Solution {
    let frames = &*FRAMES;
    // rotate piece 0 home; the search works in that frame
    let rot = coord::normalizing_rotation(state);
    let s = state.rotated(rot);
    let c: [P1; 4] = std::array::from_fn(|k| P1::from_state(&s.conjugated(frames.syms[k])));
    let d: [u8; 4] = std::array::from_fn(|k| tables.p1[c[k].index()]);

    let mut search = Search {
        tables,
        budget: d,
        best: None,
        path: vec![],
        nodes: 0,
    };
    // shortest phase 1 lengths first, so good totals prune the longer rounds
    let mut optimal = false;
    for extra in 0..=depth {
        search.budget = d.map(|x| x + extra);
        // A round only finds totals of at least its smallest budget. Once
        // that reaches the best total, no longer phase 1 can help, and since
        // solved is in every target, every solution shorter than the best
        // would have been found as a phase 1 alone: the best is optimal.
        if search.budget.iter().all(|&b| b as usize >= search.bound()) {
            optimal = true;
            break;
        }
        search.dfs(c, d, P2::from_state(&s));
    }

    let best = search.best.expect("no solution found");
    let k = best.target;
    let unsym = frames.syms[k].inverse();
    let phase2: Vec<Twist> = descend(&tables.p2, &PHASE2_MOVES, best.p2, P2::index, P2::apply)
        .into_iter()
        .map(|t| t.conjugated(unsym))
        .collect();
    let mut check = s.clone();
    check.apply_all(&best.phase1);
    check.apply_all(&phase2);
    assert_eq!(check, State::solved(), "didn't solve the normalized state");

    let back = rot.inverse();
    let to_original = |ts: &[Twist]| ts.iter().map(|t| t.conjugated(back)).collect();
    Solution {
        phase1: to_original(&best.phase1),
        phase2: to_original(&phase2),
        target: k,
        nodes: search.nodes,
        optimal,
    }
}
