//! Sticker-level puzzle state. Slow and simple; meant as the reference model.

use crate::geom::Cell;
use crate::twist::Twist;

/// The 16 piece slots are the points of {±1}⁴; slot index bit `i` is set when
/// coordinate `i` is -1. Each slot has one sticker per axis, and `stickers`
/// holds the color (home cell) of the sticker on slot `p`, axis `i` at index
/// `p * 4 + i`.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct State {
    stickers: [Cell; 64],
}

fn slot_coords(p: usize) -> [i8; 4] {
    std::array::from_fn(|i| if p >> i & 1 == 1 { -1 } else { 1 })
}
fn coords_slot(v: [i8; 4]) -> usize {
    (0..4).filter(|&i| v[i] < 0).map(|i| 1 << i).sum()
}

impl Default for State {
    fn default() -> Self {
        Self::solved()
    }
}

impl State {
    pub fn solved() -> Self {
        State {
            stickers: std::array::from_fn(|k| {
                let (p, i) = (k / 4, k % 4);
                Cell::from_axis_sign(i as u8, p >> i & 1 == 1)
            }),
        }
    }

    /// Color of the sticker on slot `p`, facing along axis `axis`.
    pub fn sticker(&self, p: usize, axis: u8) -> Cell {
        self.stickers[p * 4 + axis as usize]
    }

    pub fn apply(&mut self, t: Twist) {
        let (cell, rot) = (t.cell(), t.rot());
        let old = self.stickers;
        for p in 0..16 {
            if (p >> cell.axis() & 1 == 1) != cell.is_negative() {
                continue;
            }
            let q = coords_slot(rot.apply(slot_coords(p)));
            for i in 0..4u8 {
                let (j, _) = rot.image(i);
                self.stickers[q * 4 + j as usize] = old[p * 4 + i as usize];
            }
        }
    }

    pub fn apply_all(&mut self, twists: &[Twist]) {
        for &t in twists {
            self.apply(t);
        }
    }

    /// Stickers lying on `cell`, as (slot, axis) pairs.
    fn cell_stickers(cell: Cell) -> impl Iterator<Item = (usize, u8)> {
        (0..16)
            .filter(move |p| (p >> cell.axis() & 1 == 1) == cell.is_negative())
            .map(move |p| (p, cell.axis()))
    }

    /// Whether every cell is one color, as Hyperspeedcube judges it. This
    /// accepts any whole-puzzle orientation.
    pub fn is_solved(&self) -> bool {
        Cell::ALL.iter().all(|&c| {
            let mut it = Self::cell_stickers(c).map(|(p, a)| self.sticker(p, a));
            let first = it.next().unwrap();
            it.all(|x| x == first)
        })
    }

    /// Number of cells that are one color.
    pub fn solved_cells(&self) -> usize {
        Cell::ALL
            .iter()
            .filter(|&&c| {
                let mut it = Self::cell_stickers(c).map(|(p, a)| self.sticker(p, a));
                let first = it.next().unwrap();
                it.all(|x| x == first)
            })
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::twist::Twist;

    #[test]
    fn twist_then_inverse_is_identity() {
        for t in Twist::all() {
            let mut s = State::solved();
            s.apply(t);
            assert!(!s.is_solved(), "{t}");
            s.apply(t.inverse());
            assert_eq!(s, State::solved());
        }
    }

    #[test]
    fn twist_orders() {
        for t in Twist::all() {
            let mut s = State::solved();
            let mut n = 0;
            loop {
                s.apply(t);
                n += 1;
                if s == State::solved() {
                    break;
                }
            }
            let quarter = matches!(
                t.kind(),
                crate::twist::Kind::QuarterNext | crate::twist::Kind::QuarterPrev
            );
            assert_eq!(n, if quarter { 4 } else { 2 }, "{t}");
        }
    }

    #[test]
    fn opposite_twists_make_a_rotation() {
        // twisting both layers of an axis the same way rotates the puzzle
        let a = Twist::parse_hsc("εA_εB").unwrap();
        let c = Twist::new(a.cell().opposite(), a.rot()).unwrap();
        let mut s = State::solved();
        s.apply(a);
        s.apply(c);
        assert!(s.is_solved());
        assert_ne!(s, State::solved());
    }
}
