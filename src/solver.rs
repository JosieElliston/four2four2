//! Two-phase solver: scrambled → oll (I on I, O on O, U/D on U/D) → solved.
//! Each phase uses a complete distance table, so each phase is optimal on its
//! own; the total is not.

use std::path::Path;
use std::sync::LazyLock;

use crate::coord::{self, Coord, P1, P1Move, P2, PHASE1_MOVES, PHASE2_MOVES, SlotMove};
use crate::geom::Rot;
use crate::state::State;
use crate::table::{self, Mod3, Nibbles, child_distances};
use crate::twist::{Twist, simplify};

pub struct Tables {
    p1: Mod3,
    p2: Nibbles,
}

/// Builds a table of exact distances by BFS from distance 0.
fn bfs<C: Coord>(name: &str, size: usize, moves: &[SlotMove]) -> Vec<u8> {
    table::bfs(name, size, C::solved().index(), |i, f| {
        let c = C::from_index(i);
        for m in moves {
            f(c.apply(m).index());
        }
    })
}

impl Tables {
    /// Loads the tables from `dir`, building and caching any that are missing.
    pub fn load_or_build(dir: &Path) -> Self {
        let p1 = table::load_or_build(
            &dir.join("phase1-mod3.bin"),
            Mod3::packed_len(coord::P1_SIZE),
            || {
                Mod3::pack(&bfs::<P1>("phase 1", coord::P1_SIZE, &PHASE1_MOVES))
                    .bytes()
                    .to_vec()
            },
        );
        let p2 = table::load_or_build(
            &dir.join("phase2-nibbles.bin"),
            Nibbles::packed_len(coord::P2_SIZE),
            || {
                Nibbles::pack(&bfs::<P2>("phase 2", coord::P2_SIZE, &PHASE2_MOVES))
                    .bytes()
                    .to_vec()
            },
        );
        let (p1, p2) = (Mod3::from_bytes(p1), Nibbles::from_bytes(p2));
        Tables { p1, p2 }
    }
}

/// One step downhill from `c`: the first move whose result's table entry
/// satisfies `is_down`. Every neighbor is loaded at once so the loads overlap.
fn downhill<C: Coord>(
    moves: &[SlotMove],
    c: C,
    entry: impl Fn(usize) -> u8,
    is_down: impl Fn(u8) -> bool,
) -> (usize, C) {
    let mut next = [(c, 0); 28];
    for (j, m) in moves.iter().enumerate() {
        let n = c.apply(m);
        next[j] = (n, n.index());
    }
    let mut code = [0; 28];
    for j in 0..moves.len() {
        code[j] = entry(next[j].1);
    }
    let j = (0..moves.len())
        .find(|&j| is_down(code[j]))
        .expect("table has no downhill move");
    (j, next[j].0)
}

/// Exact phase 1 distance of `c`.
fn p1_distance(tables: &Tables, mut c: P1) -> u8 {
    let (t, goal) = (&tables.p1, P1::solved().index());
    let mut steps = 0;
    while c.index() != goal {
        // one less mod 3
        let down = (t.get(c.index()) + 2) % 3;
        c = downhill(&PHASE1_MOVES, c, |i| t.get(i), |v| v == down).1;
        steps += 1;
    }
    steps
}

/// Phase 2 distance of `c` if it's at most `max`. With `path`, also steps
/// all the way down, pushing each move.
fn p2_distance(
    tables: &Tables,
    mut c: P2,
    max: usize,
    mut path: Option<&mut Vec<Twist>>,
) -> Option<usize> {
    let t = &tables.p2;
    let exact = Nibbles::EXACT as usize;
    let mut steps = 0;
    loop {
        let code = t.get(c.index());
        if (code as usize) < exact {
            let d = steps + code as usize;
            if d > max {
                return None;
            }
            if path.is_none() {
                return Some(d);
            }
            if code == 0 {
                return Some(d);
            }
        } else if steps + exact > max {
            // still at least `exact` to go
            return None;
        }
        let (j, n) = downhill(
            &PHASE2_MOVES,
            c,
            |i| t.get(i),
            |v| Nibbles::is_downhill(code, v),
        );
        if let Some(p) = path.as_deref_mut() {
            p.push(PHASE2_MOVES[j].twist);
        }
        c = n;
        steps += 1;
    }
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
}

impl Solution {
    /// Both phases with same-cell twists across the boundary merged.
    pub fn twists(&self) -> Vec<Twist> {
        simplify(&[self.phase1.clone(), self.phase2.clone()].concat())
    }
}

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
    /// Only totals below this are wanted.
    limit: usize,
    /// Stop at the first solution found.
    first: bool,
}

impl Search<'_> {
    fn bound(&self) -> usize {
        self.best.as_ref().map_or(self.limit, |b| b.total)
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
        // only a phase 2 shorter than this helps
        let Some(limit) = self.bound().checked_sub(self.path.len() + 1) else {
            return;
        };
        if let Some(n) = p2_distance(self.tables, p2k, limit, None) {
            let total = self.path.len() + n;
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
    fn expand(
        &self,
        c: &[P1; 4],
        d: &[u8; 4],
        skip: std::ops::Range<usize>,
    ) -> ([[P1; 4]; 28], [[u8; 4]; 28]) {
        let moves = &FRAMES.moves;
        let p1 = &self.tables.p1;
        let mut cc = [[P1::solved(); 4]; 28];
        let mut dd = [[0u8; 4]; 28];
        let kids = (0..28).filter(|i| !skip.contains(i));
        let decode = d.map(child_distances);
        for i in kids.clone() {
            for k in 0..4 {
                cc[i][k] = c[k].apply_p1(&moves[i][k]);
            }
        }
        for i in kids {
            for k in 0..4 {
                dd[i][k] = decode[k][p1.get(cc[i][k].index()) as usize];
            }
        }
        (cc, dd)
    }

    /// `c[k]` is the phase 1 coordinate seen through target symmetry `k`,
    /// `d[k]` its distance. `p2` holds the pieces in the normalized frame.
    fn dfs(&mut self, c: [P1; 4], d: [u8; 4], p2: P2) {
        if self.first && self.best.is_some() {
            return;
        }
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
        let (cc, dd) = self.expand(&c, &d, skip.clone());
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

/// No solution of at most the requested length was found.
#[derive(Clone, Debug)]
pub struct NoSolution {
    /// Every phase 1 length up to the limit was searched, so none exists.
    /// False when `depth` ran out first.
    pub proved: bool,
    pub nodes: u64,
}

/// Solves `state`, trying every phase 1 up to `depth` twists longer than
/// optimal for each oll target, and keeping the shortest total. With no
/// `depth`, searches until the total is optimal. Twists in the result are in
/// the frame of `state`.
///
/// With `max`, only solutions of at most `max` twists count, and the search
/// stops at the first one found (so it need not be optimal). Twist counts here
/// are before merging across the phase boundary, so the returned solution's
/// STM may be lower still. Without `max`, there is always a solution.
pub fn solve(
    tables: &Tables,
    state: &State,
    depth: Option<u8>,
    max: Option<usize>,
) -> Result<Solution, NoSolution> {
    let frames = &*FRAMES;
    // rotate piece 0 home; the search works in that frame
    let rot = coord::normalizing_rotation(state);
    let s = state.rotated(rot);
    let c: [P1; 4] = std::array::from_fn(|k| P1::from_state(&s.conjugated(frames.syms[k])));
    let d: [u8; 4] = c.map(|c| p1_distance(tables, c));

    let mut search = Search {
        tables,
        budget: d,
        best: None,
        path: vec![],
        nodes: 0,
        limit: max.map_or(usize::MAX, |m| m + 1),
        first: max.is_some(),
    };
    // shortest phase 1 lengths first, so good totals prune the longer rounds
    let mut proved = false;
    for extra in 0..=depth.unwrap_or(u8::MAX) {
        search.budget = d.map(|x| x.saturating_add(extra));
        // A round only finds totals of at least its smallest budget. Once
        // that reaches the bound (the best total, or `max + 1`), no longer
        // phase 1 can help, and since solved is in every target, every
        // solution below the bound would have been found as a phase 1 alone:
        // the best is optimal, or there is none within `max`.
        if search.budget.iter().all(|&b| b as usize >= search.bound()) {
            proved = true;
            break;
        }
        search.dfs(c, d, P2::from_state(&s));
        if search.first && search.best.is_some() {
            break;
        }
    }

    let Some(best) = search.best else {
        return Err(NoSolution {
            proved,
            nodes: search.nodes,
        });
    };
    let k = best.target;
    let unsym = frames.syms[k].inverse();
    let mut phase2 = vec![];
    p2_distance(tables, best.p2, usize::MAX, Some(&mut phase2));
    let phase2: Vec<Twist> = phase2.into_iter().map(|t| t.conjugated(unsym)).collect();
    let mut check = s.clone();
    check.apply_all(&best.phase1);
    check.apply_all(&phase2);
    assert_eq!(check, State::solved(), "didn't solve the normalized state");

    let back = rot.inverse();
    let to_original = |ts: &[Twist]| ts.iter().map(|t| t.conjugated(back)).collect();
    Ok(Solution {
        phase1: to_original(&best.phase1),
        phase2: to_original(&phase2),
        target: k,
        nodes: search.nodes,
    })
}
