//! Twists and their HSC2 names.
//!
//! An HSC2 twist name lists the twisted cell followed by the cells the
//! rotation fixes (like clicking the sticker at that ridge or edge):
//!
//! - `εA_εB`: same-ring neighbor. Quarter turn of the *other* ring's plane:
//!   +first axis ↦ -second axis (εA_εB sends ηA to ηD). `εA_εD` is the inverse.
//!   This sign was found by replaying HSC logs (see `assets/stm35.hsc`); the other sign
//!   replays every log to garbage.
//! - `εA_ηA`: other-ring cell. 180° turn fixing both axes. `εA_ηA` and `εA_ηC`
//!   are the same twist.
//! - `εA_ηA_ηB`: adjacent pair in the other ring. 180° turn swapping the pair
//!   and negating the twisted cell's sibling axis. `εA_ηA_ηB` and `εA_ηD_ηC`
//!   are the same twist.
//!
//! The 180° same-ring turn has no HSC2 name, so it is written as two quarter
//! twists.

use std::fmt;

use crate::geom::{Cell, Rot};

/// A twist of one cell: the 8 pieces on `cell` are rotated by `rot`, which
/// fixes the cell's normal. Each cell has exactly 7 twists.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Twist {
    cell: Cell,
    rot: Rot,
}

/// The shape of a twist, relative to its cell.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Kind {
    /// Quarter turn named `X_{next(X)}`.
    QuarterNext,
    /// Quarter turn named `X_{prev(X)}`; inverse of `QuarterNext`.
    QuarterPrev,
    /// Same-ring half turn; no HSC2 name.
    Half,
    /// 180° fixing the given other-ring cell (and its opposite).
    Flip(Cell),
    /// 180° swapping the given adjacent other-ring cells.
    Edge(Cell, Cell),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ParseTwistError(pub String);

impl fmt::Display for ParseTwistError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid twist name {:?}", self.0)
    }
}
impl std::error::Error for ParseTwistError {}

/// Axes of a ring plane: (first, second).
fn ring_axes(ring: u8) -> (u8, u8) {
    (ring * 2, ring * 2 + 1)
}

impl Twist {
    /// Constructs a twist, or `None` if `rot` is not a legal twist of `cell`.
    pub fn new(cell: Cell, rot: Rot) -> Option<Self> {
        let legal = rot != Rot::IDENT
            && rot.det() == 1
            && rot.preserves_rings()
            && rot.apply_cell(cell) == cell;
        legal.then_some(Twist { cell, rot })
    }

    pub fn cell(self) -> Cell {
        self.cell
    }
    pub fn rot(self) -> Rot {
        self.rot
    }

    pub fn quarter_next(cell: Cell) -> Self {
        let (u, v) = ring_axes(1 - cell.ring());
        // u ↦ -v, v ↦ u
        let mut images = [(0, false), (1, false), (2, false), (3, false)];
        images[u as usize] = (v, true);
        images[v as usize] = (u, false);
        Twist::new(cell, Rot::from_images(images)).unwrap()
    }
    pub fn quarter_prev(cell: Cell) -> Self {
        Self::quarter_next(cell).inverse()
    }
    pub fn half(cell: Cell) -> Self {
        let q = Self::quarter_next(cell);
        Twist::new(cell, q.rot.then_after(q.rot)).unwrap()
    }
    /// 180° twist of `cell` fixing `other` (a cell in the other ring).
    pub fn flip(cell: Cell, other: Cell) -> Self {
        assert_ne!(cell.ring(), other.ring());
        let mut images = [(0, true), (1, true), (2, true), (3, true)];
        images[cell.axis() as usize] = (cell.axis(), false);
        images[other.axis() as usize] = (other.axis(), false);
        Twist::new(cell, Rot::from_images(images)).unwrap()
    }
    /// 180° twist of `cell` swapping adjacent other-ring cells `p` and `q`.
    pub fn edge(cell: Cell, p: Cell, q: Cell) -> Self {
        assert!(p.ring() == q.ring() && p.ring() != cell.ring());
        assert!(
            p.next() == q || q.next() == p,
            "{p} and {q} are not adjacent"
        );
        let sibling = cell.axis() ^ 1;
        let neg = p.is_negative() != q.is_negative();
        let mut images = [(0, false), (1, false), (2, false), (3, false)];
        images[sibling as usize] = (sibling, true);
        images[p.axis() as usize] = (q.axis(), neg);
        images[q.axis() as usize] = (p.axis(), neg);
        Twist::new(cell, Rot::from_images(images)).unwrap()
    }

    pub fn inverse(self) -> Self {
        Twist {
            cell: self.cell,
            rot: self.rot.inverse(),
        }
    }

    /// The twist `r * self * r⁻¹`: the same twist seen in a frame moved by
    /// `r`. Twisting a state's `rotated(r)` or `conjugated(r)` by
    /// `t.conjugated(r)` matches twisting the original by `t`.
    pub fn conjugated(self, r: Rot) -> Self {
        let rot = r.then_after(self.rot).then_after(r.inverse());
        Twist::new(r.apply_cell(self.cell), rot).expect("symmetry must map twists to twists")
    }

    /// The combined twist of doing `self` then `other` on the same cell, or
    /// `None` if they cancel. Panics if the cells differ.
    pub fn then(self, other: Twist) -> Option<Twist> {
        assert_eq!(self.cell, other.cell);
        Twist::new(self.cell, other.rot.then_after(self.rot))
    }

    /// All 56 twists, 7 per cell, in a fixed order.
    pub fn all() -> Vec<Twist> {
        Cell::ALL.iter().flat_map(|&c| Self::all_on(c)).collect()
    }

    /// The 7 twists of one cell.
    pub fn all_on(cell: Cell) -> [Twist; 7] {
        let o = Cell::from_ring_letter(1 - cell.ring(), 0); // other-ring A
        [
            Self::quarter_next(cell),
            Self::quarter_prev(cell),
            Self::half(cell),
            Self::flip(cell, o),
            Self::flip(cell, o.next()),
            Self::edge(cell, o, o.next()),
            Self::edge(cell, o, o.prev()),
        ]
    }

    pub fn kind(self) -> Kind {
        let c = self.cell;
        let sibling = c.axis() ^ 1;
        let (u, v) = ring_axes(1 - c.ring());
        let (_, sibling_neg) = self.rot.image(sibling);
        let (ua, un) = self.rot.image(u);
        if !sibling_neg {
            // proper rotation of the other ring's plane
            return if self == Self::quarter_next(c) {
                Kind::QuarterNext
            } else if self == Self::quarter_prev(c) {
                Kind::QuarterPrev
            } else {
                Kind::Half
            };
        }
        let o = Cell::from_ring_letter(1 - c.ring(), 0); // +u
        if ua == u {
            // reflection of the other plane fixing the u or v axis
            Kind::Flip(if un { o.next() } else { o })
        } else {
            debug_assert_eq!(ua, v);
            // u ↦ ±v: swaps +u with +v (B) or -v (D)
            Kind::Edge(o, if un { o.prev() } else { o.next() })
        }
    }

    /// HSC2 names that perform this twist, in order. One name, except for
    /// the same-ring half turn, which is two quarter twists.
    pub fn hsc_names(self) -> Vec<String> {
        let c = self.cell;
        match self.kind() {
            Kind::QuarterNext => vec![format!("{c}_{}", c.next())],
            Kind::QuarterPrev => vec![format!("{c}_{}", c.prev())],
            Kind::Half => vec![format!("{c}_{}", c.next()); 2],
            Kind::Flip(o) => vec![format!("{c}_{o}")],
            Kind::Edge(p, q) => vec![format!("{c}_{p}_{q}")],
        }
    }

    /// Parses one HSC2 twist name. Accepts every name HSC2 uses, including
    /// the aliases (`εA_ηC` for `εA_ηA`, `εA_ηD_ηC` for `εA_ηA_ηB`), and edge
    /// pairs in either order.
    pub fn parse_hsc(name: &str) -> Result<Self, ParseTwistError> {
        let err = || ParseTwistError(name.to_owned());
        let cells: Vec<Cell> = name
            .split('_')
            .map(Cell::parse)
            .collect::<Option<_>>()
            .ok_or_else(err)?;
        match cells[..] {
            [c, y] if y.ring() == c.ring() => {
                if y == c.next() {
                    Ok(Self::quarter_next(c))
                } else if y == c.prev() {
                    Ok(Self::quarter_prev(c))
                } else {
                    Err(err())
                }
            }
            [c, y] => Ok(Self::flip(c, y)),
            [c, p, q]
                if p.ring() == q.ring()
                    && p.ring() != c.ring()
                    && (p.next() == q || q.next() == p) =>
            {
                Ok(Self::edge(c, p, q))
            }
            _ => Err(err()),
        }
    }
}

impl fmt::Display for Twist {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.hsc_names().join(" "))
    }
}

/// Parses whitespace-separated HSC2 twist names.
pub fn parse_hsc_twists(s: &str) -> Result<Vec<Twist>, ParseTwistError> {
    s.split_whitespace().map(Twist::parse_hsc).collect()
}

/// Merges consecutive twists of the same cell, dropping runs that cancel.
/// The result has no two adjacent twists on the same cell.
pub fn simplify(twists: &[Twist]) -> Vec<Twist> {
    let mut out: Vec<Twist> = vec![];
    for &t in twists {
        match out.last() {
            Some(&last) if last.cell() == t.cell() => {
                out.pop();
                out.extend(last.then(t));
            }
            _ => out.push(t),
        }
    }
    out
}

/// Slice turn metric as Hyperspeedcube counts it: the number of maximal runs
/// of consecutive twists on the same cell. A run that cancels out still
/// counts. This matches the `stmN` in HSC's log filenames.
pub fn hsc_stm(twists: &[Twist]) -> usize {
    twists
        .iter()
        .enumerate()
        .filter(|&(i, t)| i == 0 || twists[i - 1].cell() != t.cell())
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn seven_distinct_twists_per_cell() {
        let all = Twist::all();
        assert_eq!(all.iter().collect::<HashSet<_>>().len(), 56);
        for t in &all {
            assert_eq!(t.rot().det(), 1);
            assert!(t.rot().preserves_rings());
        }
    }

    #[test]
    fn names_round_trip() {
        for t in Twist::all() {
            let parsed = parse_hsc_twists(&t.to_string()).unwrap();
            let combined = parsed
                .iter()
                .skip(1)
                .try_fold(parsed[0], |acc, &x| acc.then(x));
            assert_eq!(combined, Some(t), "{t}");
        }
    }

    #[test]
    fn every_hsc_name_parses() {
        // all 80 names HSC2 uses for this puzzle: 10 per cell
        let mut names = HashSet::new();
        for c in Cell::ALL {
            names.insert(format!("{c}_{}", c.next()));
            names.insert(format!("{c}_{}", c.prev()));
            for l in 0..4 {
                let o = Cell::from_ring_letter(1 - c.ring(), l);
                names.insert(format!("{c}_{o}"));
                names.insert(format!("{c}_{o}_{}", o.next()));
            }
        }
        assert_eq!(names.len(), 80);
        let twists: HashSet<_> = names.iter().map(|n| Twist::parse_hsc(n).unwrap()).collect();
        assert_eq!(twists.len(), 48); // everything except the 8 half turns
    }

    #[test]
    fn aliases_agree() {
        let p = |s| Twist::parse_hsc(s).unwrap();
        assert_eq!(p("εA_ηA"), p("εA_ηC"));
        assert_eq!(p("εA_ηA_ηB"), p("εA_ηD_ηC"));
        assert_eq!(p("εA_ηA_ηB"), p("εA_ηB_ηA"));
        assert_eq!(p("εA_ηA_ηD"), p("εA_ηB_ηC"));
        assert_eq!(p("εA_εB").inverse(), p("εA_εD"));
        assert!(Twist::parse_hsc("εA_εC").is_err());
        assert!(Twist::parse_hsc("εA_ηA_ηC").is_err());
        assert!(Twist::parse_hsc("εA_εB_εC").is_err());
    }

    #[test]
    fn quarter_direction() {
        // verified against HSC logs: εA_εB sends ηA (+z) to ηD (-w)
        let t = Twist::parse_hsc("εA_εB").unwrap();
        assert_eq!(
            t.rot().apply_cell(Cell::parse("ηA").unwrap()),
            Cell::parse("ηD").unwrap()
        );
    }

    #[test]
    fn simplify_and_stm() {
        let ts = parse_hsc_twists("εA_εB εA_εB ηA_εB εA_εB εA_εD ηA_εB").unwrap();
        assert_eq!(hsc_stm(&ts), 4);
        let s = simplify(&ts);
        // εA half, then ηA_εB, (cancelled εA run), ηA_εB → the two ηA flips cancel too
        assert_eq!(s, vec![Twist::half(Cell::parse("εA").unwrap())]);
    }
}
