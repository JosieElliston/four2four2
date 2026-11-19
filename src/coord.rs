//! Coordinates for the two phases, in the frame where the piece whose home is
//! slot 0 (all coordinates +1) is held fixed.
//!
//! Only twists of the four negative cells (εC, εD, ηC, ηD) are used. They never
//! move slot 0, and a twist of a positive cell is the same as the matching
//! negative-cell twist followed by a whole-puzzle rotation, so nothing is lost.
//!
//! Slots are indexed as in [`State`]: bit `i` set when coordinate `i` is -1.
//! "z" is axis 2 (ηA/ηC), "w" is axis 3 (ηB/ηD).

use std::sync::LazyLock;

use crate::geom::{Cell, Rot};
use crate::state::State;
use crate::twist::Twist;

/// The effect of a twist on slots.
#[derive(Clone, Debug)]
pub struct SlotMove {
    pub twist: Twist,
    /// `perm[p]` is where slot `p`'s piece goes.
    pub perm: [u8; 16],
    /// Whether the twist swaps the z and w axes (flipping η orientation).
    pub swaps_zw: bool,
}

impl SlotMove {
    fn new(twist: Twist) -> Self {
        let (cell, rot) = (twist.cell(), twist.rot());
        let perm = std::array::from_fn(|p| {
            if (p >> cell.axis() & 1 == 1) == cell.is_negative() {
                slot_of(rot.apply(coords_of(p))) as u8
            } else {
                p as u8
            }
        });
        SlotMove {
            twist,
            perm,
            swaps_zw: rot.image(2).0 == 3,
        }
    }
}

pub fn coords_of(p: usize) -> [i8; 4] {
    std::array::from_fn(|i| if p >> i & 1 == 1 { -1 } else { 1 })
}
pub fn slot_of(v: [i8; 4]) -> usize {
    (0..4).filter(|&i| v[i] < 0).map(|i| 1 << i).sum()
}

fn negative_cell_twists() -> impl Iterator<Item = Twist> {
    Cell::ALL
        .into_iter()
        .filter(|c| c.is_negative())
        .flat_map(Twist::all_on)
}

/// All 28 twists usable in the fixed frame.
pub static PHASE1_MOVES: LazyLock<Vec<SlotMove>> =
    LazyLock::new(|| negative_cell_twists().map(SlotMove::new).collect());

/// The 12 fixed-frame twists that keep I on I, O on O, U/D on U/D: those that
/// fix the z axis exactly.
pub static PHASE2_MOVES: LazyLock<Vec<SlotMove>> = LazyLock::new(|| {
    negative_cell_twists()
        .filter(|t| t.rot().image(2) == (2, false))
        .map(SlotMove::new)
        .collect()
});

/// Home slot of the piece at slot `p`.
pub fn piece_at(s: &State, p: usize) -> usize {
    (0..4u8)
        .map(|i| s.sticker(p, i))
        .filter(|c| c.is_negative())
        .map(|c| 1 << c.axis())
        .sum()
}

/// The rotation that brings the piece whose home is slot 0 back home.
pub fn normalizing_rotation(s: &State) -> Rot {
    let p = (0..16).find(|&p| piece_at(s, p) == 0).unwrap();
    let v = coords_of(p);
    let images = std::array::from_fn(|i| (s.sticker(p, i as u8).axis(), v[i] < 0));
    let r = Rot::from_images(images);
    assert_eq!(r.det(), 1, "state is mirrored");
    r
}

// ---------- phase 1: orientation and I/O separation ----------

/// C(15, 8): ways to place the 8 O-type pieces in slots 1..16.
pub const P1_SEP: usize = 6435;
/// One η-orientation bit for each of slots 1..16.
pub const P1_ORI: usize = 1 << 15;
pub const P1_SIZE: usize = P1_SEP * P1_ORI;

struct SepTables {
    rank: Vec<u16>,   // 15-bit mask -> rank, or u16::MAX
    unrank: Vec<u16>, // rank -> mask
}

static SEP: LazyLock<SepTables> = LazyLock::new(|| {
    let mut rank = vec![u16::MAX; 1 << 15];
    let mut unrank = vec![];
    for m in 0..1u32 << 15 {
        if m.count_ones() == 8 {
            rank[m as usize] = unrank.len() as u16;
            unrank.push(m as u16);
        }
    }
    assert_eq!(unrank.len(), P1_SEP);
    SepTables { rank, unrank }
});

/// Phase 1 data per slot, as two 16-bit masks (bit 0 = slot 0, always 0):
/// `o`: the piece is O-type (home has z = -1);
/// `ori`: the sticker on the slot's z axis is a w-colored (U/D) sticker.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct P1 {
    pub o: u16,
    pub ori: u16,
}

impl P1 {
    pub fn from_state(s: &State) -> Self {
        let (mut o, mut ori) = (0, 0);
        for p in 1..16 {
            if piece_at(s, p) >> 2 & 1 == 1 {
                o |= 1 << p;
            }
            if s.sticker(p, 2).axis() != 2 {
                ori |= 1 << p;
            }
        }
        P1 { o, ori }
    }

    pub fn solved() -> Self {
        let o = (1..16).filter(|p| p >> 2 & 1 == 1).map(|p| 1 << p).sum();
        P1 { o, ori: 0 }
    }

    pub fn index(self) -> usize {
        let sep = SEP.rank[(self.o >> 1) as usize];
        debug_assert_ne!(sep, u16::MAX);
        sep as usize * P1_ORI + (self.ori >> 1) as usize
    }

    pub fn from_index(i: usize) -> Self {
        P1 {
            o: SEP.unrank[i / P1_ORI] << 1,
            ori: ((i % P1_ORI) as u16) << 1,
        }
    }

    pub fn apply(self, m: &SlotMove) -> Self {
        let (mut o, mut ori) = (0, 0);
        for p in 0..16 {
            let q = m.perm[p];
            let flip = (m.swaps_zw && moves_slot(m, p)) as u16;
            o |= (self.o >> p & 1) << q;
            ori |= ((self.ori >> p & 1) ^ flip) << q;
        }
        P1 { o, ori }
    }
}

/// Whether slot `p` is on the twisted cell (even if it maps to itself).
fn moves_slot(m: &SlotMove, p: usize) -> bool {
    let c = m.twist.cell();
    (p >> c.axis() & 1 == 1) == c.is_negative()
}

// ---------- phase 2: permutation within the z+ and z- groups ----------

/// z+ slots other than slot 0, and z- slots.
pub const P2_BLOCK_A: [u8; 7] = [1, 2, 3, 8, 9, 10, 11];
pub const P2_BLOCK_B: [u8; 8] = [4, 5, 6, 7, 12, 13, 14, 15];
pub const P2_SIZE: usize = 5040 * 40320;

/// Which piece (by home slot) sits at each slot.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct P2 {
    pub pieces: [u8; 16],
}

fn block_pos(slot: u8) -> usize {
    P2_BLOCK_A
        .iter()
        .position(|&x| x == slot)
        .or_else(|| P2_BLOCK_B.iter().position(|&x| x == slot))
        .unwrap()
}

fn rank_perm(perm: &[u8]) -> usize {
    let n = perm.len();
    let mut r = 0;
    for i in 0..n {
        let smaller = perm[i + 1..].iter().filter(|&&x| x < perm[i]).count();
        r = r * (n - i) + smaller;
    }
    r
}

fn unrank_perm<const N: usize>(mut r: usize) -> [u8; N] {
    let mut digits = [0usize; N];
    for i in (0..N).rev() {
        digits[i] = r % (N - i);
        r /= N - i;
    }
    let mut avail: Vec<u8> = (0..N as u8).collect();
    std::array::from_fn(|i| avail.remove(digits[i]))
}

impl P2 {
    pub fn from_state(s: &State) -> Self {
        P2 {
            pieces: std::array::from_fn(|p| piece_at(s, p) as u8),
        }
    }

    pub fn solved() -> Self {
        P2 {
            pieces: std::array::from_fn(|p| p as u8),
        }
    }

    pub fn index(self) -> usize {
        let a: [u8; 7] =
            std::array::from_fn(|i| block_pos(self.pieces[P2_BLOCK_A[i] as usize]) as u8);
        let b: [u8; 8] =
            std::array::from_fn(|i| block_pos(self.pieces[P2_BLOCK_B[i] as usize]) as u8);
        rank_perm(&a) * 40320 + rank_perm(&b)
    }

    pub fn from_index(i: usize) -> Self {
        let a: [u8; 7] = unrank_perm(i / 40320);
        let b: [u8; 8] = unrank_perm(i % 40320);
        let mut pieces = [0; 16];
        for k in 0..7 {
            pieces[P2_BLOCK_A[k] as usize] = P2_BLOCK_A[a[k] as usize];
        }
        for k in 0..8 {
            pieces[P2_BLOCK_B[k] as usize] = P2_BLOCK_B[b[k] as usize];
        }
        P2 { pieces }
    }

    pub fn apply(self, m: &SlotMove) -> Self {
        let mut pieces = [0; 16];
        for p in 0..16 {
            pieces[m.perm[p] as usize] = self.pieces[p];
        }
        P2 { pieces }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn random_state(seed: u64, n: usize) -> State {
        let mut x = seed;
        let all = Twist::all();
        let mut s = State::solved();
        for _ in 0..n {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            s.apply(all[(x % all.len() as u64) as usize]);
        }
        s
    }

    #[test]
    fn move_counts() {
        assert_eq!(PHASE1_MOVES.len(), 28);
        assert_eq!(PHASE2_MOVES.len(), 12);
    }

    #[test]
    fn p1_tracks_state() {
        let s = random_state(1, 200);
        let mut s = s.rotated(normalizing_rotation(&s));
        let mut c = P1::from_state(&s);
        for k in 0..300 {
            let m = &PHASE1_MOVES[(k * 11 + 3) % 28];
            s.apply(m.twist);
            c = c.apply(m);
            assert_eq!(c, P1::from_state(&s));
            assert_eq!(P1::from_index(c.index()), c);
        }
    }

    #[test]
    fn p2_tracks_state() {
        let mut s = State::solved();
        let mut c = P2::from_state(&s);
        assert_eq!(c, P2::solved());
        for k in 0..300 {
            let m = &PHASE2_MOVES[(k * 5 + 1) % 12];
            s.apply(m.twist);
            c = c.apply(m);
            assert_eq!(c, P2::from_state(&s));
            assert_eq!(P2::from_index(c.index()), c);
            assert_eq!(P1::from_state(&s), P1::solved());
        }
    }

    #[test]
    fn normalizing_fixes_piece_zero() {
        for seed in 1..20 {
            let s = random_state(seed, 100);
            let n = s.rotated(normalizing_rotation(&s));
            for i in 0..4 {
                assert_eq!(n.sticker(0, i), Cell::from_axis_sign(i, false));
            }
            assert_eq!(n.is_solved(), s.is_solved());
        }
    }
}
