//! Phase 1 grip table: for each phase 1 coordinate, which twists lower its
//! distance and which raise it, so the search learns every child's distance
//! from one load instead of 28.
//!
//! A cell's 8 rotations (the identity and its 7 twists) form a group, so the
//! 8 positions they reach from `c` are pairwise one twist apart, and their
//! distances take at most two adjacent values. A cell's byte says which of
//! them are at the higher value: bit 0 for `c` itself, bit `1 + t` for twist
//! `t` of the cell in [`PHASE1_MOVES`]. If `c` is high, the twists reaching
//! low positions lower the distance; if `c` is low, the twists reaching high
//! positions raise it. (So a cell has 255 distinct patterns: all 256 bytes,
//! with all-low and all-high both meaning no change.)
//!
//! The x <-> y mirror ([`P1::swap_xy`]) fixes slot 0 and the oll target, so a
//! position's entry is its mirror's with the twists mirrored. Only one O-mask
//! of each mirror pair is stored (and every O-mask the mirror fixes): 3379
//! of the 6435.

use rayon::prelude::*;

use crate::coord::{P1, P1_ORI, P1_SEP, PHASE1_MOVES};
use crate::geom::Rot;
use crate::table::Mod3;

/// O-masks stored: the mirror fixes 323 of the 6435, and pairs up the rest.
pub const BLOCKS: usize = (P1_SEP + 323) / 2;
/// Bytes in the table: one per cell per stored coordinate.
pub const LEN: usize = BLOCKS * P1_ORI * 4;

/// The x <-> y mirror, as a symmetry for conjugating twists.
pub fn mirror() -> Rot {
    Rot::from_images([(1, false), (0, false), (2, false), (3, false)])
}

pub struct Grips {
    /// `block[sep]`: where a stored O-mask's entries start, in units of
    /// [`P1_ORI`]; unused for O-masks that aren't stored.
    block: Vec<u16>,
    /// `mirror_sep[sep]`: the rank of the mirrored O-mask.
    mirror_sep: Vec<u16>,
    bytes: Vec<u8>,
}

/// The O-mask ranks that are stored (the smaller of each mirror pair), and
/// each rank's mirror.
fn layout() -> (Vec<usize>, Vec<u16>) {
    let mirror_sep: Vec<u16> = (0..P1_SEP)
        .map(|r| P1::from_index(r * P1_ORI).swap_xy().sep() as u16)
        .collect();
    let stored: Vec<usize> = (0..P1_SEP).filter(|&r| r <= mirror_sep[r] as usize).collect();
    assert_eq!(stored.len(), BLOCKS);
    (stored, mirror_sep)
}

impl Grips {
    /// Builds the table from the exact-up-to-mod-3 phase 1 table.
    pub fn build(p1: &Mod3) -> Vec<u8> {
        let (stored, _) = layout();
        let mut bytes = vec![0; LEN];
        bytes
            .par_chunks_mut(4 * P1_ORI)
            .zip(&stored)
            .for_each(|(out, &r)| {
                for (o, e) in out.chunks_exact_mut(4).enumerate() {
                    let c = P1::from_index(r * P1_ORI + o);
                    let v = p1.get(c.index());
                    for (cell, b) in e.iter_mut().enumerate() {
                        let (mut lower, mut raise) = (0u8, 0u8);
                        for t in 0..7 {
                            let w = p1.get(c.apply_p1(&PHASE1_MOVES[7 * cell + t].p1).index());
                            lower |= ((w == (v + 2) % 3) as u8) << t;
                            raise |= ((w == (v + 1) % 3) as u8) << t;
                        }
                        assert!(lower == 0 || raise == 0, "a cell both lowers and raises");
                        *b = if lower != 0 { 1 | (!lower & 0x7f) << 1 } else { raise << 1 };
                    }
                }
            });
        bytes
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        assert_eq!(bytes.len(), LEN);
        let (stored, mirror_sep) = layout();
        let mut block = vec![u16::MAX; P1_SEP];
        for (b, &r) in stored.iter().enumerate() {
            block[r] = b as u16;
        }
        Grips {
            block,
            mirror_sep,
            bytes,
        }
    }

    /// Entry index of `c`, and whether it's stored as its mirror. Branchless:
    /// which of the two is stored is a coin flip, and it decides the load's
    /// address.
    #[inline]
    fn key(&self, c: P1) -> (usize, bool) {
        let r = c.sep();
        let rm = self.mirror_sep[r] as usize;
        let mirrored = r > rm;
        let m = 0usize.wrapping_sub(mirrored as usize);
        let ori = c.ori_index() & !m | c.swap_xy().ori_index() & m;
        let block = self.block[r & !m | rm & m] as usize;
        (block * P1_ORI + ori, mirrored)
    }

    /// The four cell bytes of `c` (cell `i` in byte `i`), in the frame of
    /// `c`'s mirror when the flag is set.
    #[inline]
    pub fn get(&self, c: P1) -> (u32, bool) {
        let (i, mirrored) = self.key(c);
        let b = &self.bytes[4 * i..4 * i + 4];
        (u32::from_le_bytes(b.try_into().unwrap()), mirrored)
    }

    /// Starts loading `c`'s entry.
    #[inline]
    pub fn prefetch(&self, c: P1) {
        let p = self.bytes.as_ptr().wrapping_add(4 * self.key(c).0);
        #[cfg(target_arch = "aarch64")]
        unsafe {
            std::arch::asm!("prfm pldl1keep, [{0}]", in(reg) p, options(nostack, readonly, preserves_flags));
        }
        #[cfg(target_arch = "x86_64")]
        unsafe {
            use std::arch::x86_64::{_MM_HINT_T0, _mm_prefetch};
            _mm_prefetch::<_MM_HINT_T0>(p as *const i8);
        }
        #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
        let _ = p;
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coord::normalizing_rotation;
    use crate::state::State;
    use crate::twist::Twist;

    #[test]
    fn swap_xy_is_the_mirror() {
        let all = Twist::all();
        let mut s = State::solved();
        for k in 0..300 {
            s.apply(all[(k * 37 + 11) % all.len()]);
            let n = s.rotated(normalizing_rotation(&s));
            let c = P1::from_state(&n);
            assert_eq!(P1::from_state(&n.conjugated(mirror())), c.swap_xy());
            assert_eq!(c.swap_xy().swap_xy(), c);
        }
    }

    #[test]
    fn mirror_fixed_count() {
        let fixed = (0..P1_SEP)
            .filter(|&r| P1::from_index(r * P1_ORI).swap_xy().sep() == r)
            .count();
        assert_eq!(fixed, 323);
    }
}
