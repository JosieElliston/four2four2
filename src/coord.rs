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
    /// The twist's effect on a [`P1`].
    pub p1: P1Move,
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
        let swaps_zw = rot.image(2).0 == 3;
        SlotMove {
            twist,
            perm,
            swaps_zw,
            p1: P1Move::new(cell, rot, swaps_zw),
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

/// A phase's coordinate: what its table is indexed by.
pub trait Coord: Copy {
    /// Distance 0.
    fn solved() -> Self;
    fn index(self) -> usize;
    fn from_index(i: usize) -> Self;
    fn apply(self, m: &SlotMove) -> Self;
}

macro_rules! impl_coord {
    ($t:ty) => {
        impl Coord for $t {
            fn solved() -> Self {
                <$t>::solved()
            }
            #[inline]
            fn index(self) -> usize {
                <$t>::index(self)
            }
            fn from_index(i: usize) -> Self {
                <$t>::from_index(i)
            }
            #[inline]
            fn apply(self, m: &SlotMove) -> Self {
                <$t>::apply(self, m)
            }
        }
    };
}
impl_coord!(P1);
impl_coord!(P2);

// ---------- phase 1: orientation and I/O separation ----------

/// C(15, 8): ways to place the 8 O-type pieces in slots 1..16.
pub const P1_SEP: usize = 6435;
/// One η-orientation bit for each of slots 1..15. The bit for slot 15 is
/// implied: every reachable position has an even number of bits set.
pub const P1_ORI: usize = 1 << 14;
pub const P1_SIZE: usize = P1_SEP * P1_ORI;

const fn binomial(n: u32, k: u32) -> u32 {
    if k > n {
        return 0;
    }
    let (mut r, mut i) = (1, 0);
    while i < k {
        r = r * (n - i) / (i + 1);
        i += 1;
    }
    r
}

/// The rank of an 8-bit-set 15-bit O-mask (slots 1..16) among all of them in
/// increasing order is its combinatorial number, `sum C(p_j, j + 1)` over
/// its set bits `p_0 < p_1 < ...`. The low byte's share depends only on the
/// low byte; the rest depends on the high bits and how many bits the low byte
/// has. Splitting it this way keeps the tables small enough to stay in L1
/// while the search streams the phase 1 table past them.
///
/// `SEP_LO[lo]`: low 16 bits, the low byte's share; high bits, its popcount
/// times 128 (the offset into `SEP_HI`).
static SEP_LO: [u32; 256] = {
    let mut t = [0; 256];
    let mut lo = 0;
    while lo < 256 {
        let (mut r, mut c, mut p) = (0, 0, 0);
        while p < 8 {
            if lo >> p & 1 == 1 {
                c += 1;
                r += binomial(p, c);
            }
            p += 1;
        }
        t[lo as usize] = r | (c * 128) << 16;
        lo += 1;
    }
    t
};

/// `SEP_HI[c * 128 + hi]`: the share of the high 7 bits `hi`, given `c` bits
/// below them.
static SEP_HI: [u16; 9 * 128] = {
    let mut t = [0; 9 * 128];
    let mut c0 = 0;
    while c0 <= 8 {
        let mut hi = 0;
        while hi < 128 {
            let (mut r, mut c, mut p) = (0, c0, 0);
            while p < 7 {
                if hi >> p & 1 == 1 {
                    c += 1;
                    r += binomial(p + 8, c);
                }
                p += 1;
            }
            t[(c0 * 128 + hi) as usize] = r as u16;
            hi += 1;
        }
        c0 += 1;
    }
    t
};

/// Rank of a 15-bit O-mask with 8 bits set.
#[inline]
fn sep_rank(m: u32) -> u32 {
    let lo = SEP_LO[(m & 0xFF) as usize];
    (lo & 0xFFFF) + SEP_HI[((lo >> 16) + (m >> 8 & 0x7F)) as usize] as u32
}

/// Rank -> 15-bit O-mask, in increasing order.
static SEP_UNRANK: [u16; P1_SEP] = {
    let mut unrank = [0; P1_SEP];
    let (mut m, mut r) = (0, 0);
    while m < 1 << 15 {
        if (m as u32).count_ones() == 8 {
            unrank[r] = m as u16;
            r += 1;
        }
        m += 1;
    }
    assert!(r == P1_SEP);
    unrank
};

/// Phase 1 data per slot, as two 16-bit masks packed in one word (bit 0 =
/// slot 0, always 0):
/// - bits 0..16, `o`: the piece is O-type (home has z = -1);
/// - bits 16..32, `ori`: the sticker on the slot's z axis is a w-colored (U/D)
///   sticker.
///
/// Both masks are indexed by slot, so a twist permutes their bits the same
/// way, and both move together.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct P1(u32);

/// Number of delta swaps in a [`P1Move`].
const P1_SWAPS: usize = 2;

/// A twist's effect on [`P1`]. A twist moves the slots of its cell by a
/// signed permutation of the axes, which on slot indices is a permutation of
/// index bits after an xor. Negating index bit `i` and swapping index bits
/// `i` and `j` each permute a slot mask by one delta swap, so the whole twist
/// is a few delta swaps (restricted to the cell), then an xor of `ori` on the
/// cell when z and w trade places.
#[derive(Copy, Clone, Debug)]
pub struct P1Move {
    /// `(mask, shift)`: swap bit `p` with bit `p + shift` for each `p` in
    /// `mask`. Unused ones are `(0, 0)`.
    swaps: [(u32, u32); P1_SWAPS],
    flip: u32,
}

impl P1Move {
    fn new(cell: Cell, rot: Rot, swaps_zw: bool) -> Self {
        let in_cell = |p: usize| (p >> cell.axis() & 1 == 1) == cell.is_negative();
        let mask = |f: &dyn Fn(usize) -> bool| -> u32 {
            let m: u32 = (0..16)
                .filter(|&p| in_cell(p) && f(p))
                .map(|p| 1 << p)
                .sum();
            m | m << 16
        };
        let mut perm: [usize; 4] = std::array::from_fn(|i| rot.image(i as u8).0 as usize);
        let mut neg: [bool; 4] = std::array::from_fn(|i| rot.image(i as u8).1);
        let mut swaps = vec![];
        // new index bit perm[i] is old index bit i xor neg[i]. Swapping two
        // index bits and negating both is one delta swap, between the slots
        // where both bits are 0 and those where both are 1...
        for i in 0..4 {
            let j = perm[i];
            if i < j && perm[j] == i && neg[i] && neg[j] {
                swaps.push((
                    mask(&|p| p >> i & 1 == 0 && p >> j & 1 == 0),
                    (1 << i) + (1 << j),
                ));
                (perm[i], perm[j], neg[i], neg[j]) = (i, j, false, false);
            }
        }
        // ...then the remaining xor...
        for (i, &n) in neg.iter().enumerate() {
            if n {
                swaps.push((mask(&|p| p >> i & 1 == 0), 1 << i));
            }
        }
        // ...then the remaining bit permutation, as transpositions of index
        // bits. The ops above touch disjoint index bits, so they commute.
        for i in 0..4 {
            while perm[i] != i {
                let j = perm[i];
                let (a, b) = (i.min(j), i.max(j));
                swaps.push((
                    mask(&|p| p >> a & 1 == 1 && p >> b & 1 == 0),
                    (1 << b) - (1 << a),
                ));
                perm.swap(i, j);
            }
        }
        assert!(swaps.len() <= P1_SWAPS, "{} swaps", swaps.len());
        swaps.resize(P1_SWAPS, (0, 0));
        P1Move {
            swaps: swaps.try_into().unwrap(),
            flip: if swaps_zw {
                mask(&|_| true) & 0xFFFF_0000
            } else {
                0
            },
        }
    }
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
        P1(o | ori << 16)
    }

    pub fn solved() -> Self {
        let o: u32 = (1..16).filter(|p| p >> 2 & 1 == 1).map(|p| 1 << p).sum();
        P1(o)
    }

    pub fn o(self) -> u16 {
        self.0 as u16
    }
    pub fn ori(self) -> u16 {
        (self.0 >> 16) as u16
    }

    #[inline]
    pub fn index(self) -> usize {
        debug_assert_eq!(self.o().count_ones(), 8);
        debug_assert!(self.ori().count_ones().is_multiple_of(2));
        sep_rank(self.0 >> 1 & 0x7FFF) as usize * P1_ORI + (self.0 >> 17 & 0x3FFF) as usize
    }

    pub fn from_index(i: usize) -> Self {
        let o = (SEP_UNRANK[i / P1_ORI] as u32) << 1;
        let ori = (i % P1_ORI) as u32;
        let ori = (ori | (ori.count_ones() & 1) << 14) << 1;
        P1(o | ori << 16)
    }

    #[inline]
    pub fn apply(self, m: &SlotMove) -> Self {
        self.apply_p1(&m.p1)
    }

    #[inline]
    pub fn apply_p1(self, m: &P1Move) -> Self {
        let mut x = self.0;
        for &(mask, shift) in &m.swaps {
            let t = (x >> shift ^ x) & mask;
            x ^= t ^ t << shift;
        }
        P1(x ^ m.flip)
    }
}

// ---------- phase 2: permutation within the z+ and z- groups ----------

/// z+ slots other than slot 0, and z- slots.
pub const P2_BLOCK_A: [u8; 7] = [1, 2, 3, 8, 9, 10, 11];
pub const P2_BLOCK_B: [u8; 8] = [4, 5, 6, 7, 12, 13, 14, 15];
/// Only even permutations (of the 15 pieces) are reachable, so the z- block's
/// parity is implied by the z+ block's, and its rank's last Lehmer digit (its
/// lowest bit) is dropped.
pub const P2_SIZE: usize = 5040 * 20160;

/// Which piece (by home slot) sits at each slot.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct P2 {
    pub pieces: [u8; 16],
}

/// `BLOCK_POS[slot]`: the slot's position within its block.
static BLOCK_POS: [u8; 16] = {
    let mut t = [u8::MAX; 16];
    let mut i = 0;
    while i < 8 {
        if i < 7 {
            t[P2_BLOCK_A[i] as usize] = i as u8;
        }
        t[P2_BLOCK_B[i] as usize] = i as u8;
        i += 1;
    }
    t
};

/// Lehmer code: digit `i` counts the later entries smaller than `perm[i]`,
/// which is `perm[i]` minus the earlier entries smaller than it.
#[inline]
fn rank_perm(perm: &[u8]) -> usize {
    let n = perm.len();
    let (mut r, mut seen) = (0, 0u32);
    for (i, &x) in perm.iter().enumerate() {
        let smaller = x as u32 - (seen & ((1 << x) - 1)).count_ones();
        seen |= 1 << x;
        r = r * (n - i) + smaller as usize;
    }
    r
}

/// Whether a permutation is odd.
fn odd(perm: &[u8]) -> bool {
    let mut inversions = 0;
    for i in 0..perm.len() {
        inversions += perm[i + 1..].iter().filter(|&&x| x < perm[i]).count();
    }
    inversions % 2 == 1
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

    #[cfg_attr(feature = "profiling", inline(never))]
    pub fn index(self) -> usize {
        let a: [u8; 7] = P2_BLOCK_A.map(|p| BLOCK_POS[self.pieces[p as usize] as usize]);
        let b: [u8; 8] = P2_BLOCK_B.map(|p| BLOCK_POS[self.pieces[p as usize] as usize]);
        debug_assert!(!(odd(&a) ^ odd(&b)));
        rank_perm(&a) * 20160 + rank_perm(&b) / 2
    }

    pub fn from_index(i: usize) -> Self {
        let a: [u8; 7] = unrank_perm(i / 20160);
        let mut b: [u8; 8] = unrank_perm(i % 20160 * 2);
        if odd(&a) ^ odd(&b) {
            b = unrank_perm(i % 20160 * 2 + 1);
        }
        let mut pieces = [0; 16];
        for k in 0..7 {
            pieces[P2_BLOCK_A[k] as usize] = P2_BLOCK_A[a[k] as usize];
        }
        for k in 0..8 {
            pieces[P2_BLOCK_B[k] as usize] = P2_BLOCK_B[b[k] as usize];
        }
        P2 { pieces }
    }

    #[cfg_attr(feature = "profiling", inline(never))]
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
    fn sep_rank_is_position_in_order() {
        for (r, &m) in SEP_UNRANK.iter().enumerate() {
            assert_eq!(sep_rank(m as u32), r as u32, "{m:015b}");
        }
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
