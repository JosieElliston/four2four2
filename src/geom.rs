//! Cells and 4D signed-permutation rotations.
//!
//! Coordinates: x, y span the ε ring and z, w span the η ring. Within a ring,
//! letters go around as A = +first axis, B = +second, C = -first, D = -second,
//! so εA = +x, εB = +y, εC = -x, εD = -y, ηA = +z, ηB = +w, ηC = -z, ηD = -w.

use std::fmt;

/// One of the 8 cells (facets). Index = `axis * 2 + (negative as u8)`.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct Cell(u8);

pub const RING_CHARS: [char; 2] = ['ε', 'η'];
pub const LETTERS: [char; 4] = ['A', 'B', 'C', 'D'];

impl Cell {
    pub const ALL: [Cell; 8] = [
        Cell(0),
        Cell(1),
        Cell(2),
        Cell(3),
        Cell(4),
        Cell(5),
        Cell(6),
        Cell(7),
    ];

    pub const fn from_index(i: u8) -> Self {
        assert!(i < 8);
        Cell(i)
    }
    pub const fn index(self) -> u8 {
        self.0
    }
    /// Coordinate axis (0..4) normal to this cell.
    pub const fn axis(self) -> u8 {
        self.0 >> 1
    }
    pub const fn is_negative(self) -> bool {
        self.0 & 1 == 1
    }
    /// 0 for ε, 1 for η.
    pub const fn ring(self) -> u8 {
        self.axis() >> 1
    }
    /// Position around the ring: 0..4 for A..D.
    pub const fn letter(self) -> u8 {
        (self.axis() & 1) + 2 * (self.0 & 1)
    }
    pub const fn from_ring_letter(ring: u8, letter: u8) -> Self {
        assert!(ring < 2 && letter < 4);
        let axis = ring * 2 + (letter & 1);
        Cell(axis * 2 + (letter >> 1))
    }
    pub const fn from_axis_sign(axis: u8, negative: bool) -> Self {
        Cell(axis * 2 + negative as u8)
    }
    /// Next cell around the ring (A→B→C→D→A).
    pub const fn next(self) -> Self {
        Self::from_ring_letter(self.ring(), (self.letter() + 1) % 4)
    }
    pub const fn prev(self) -> Self {
        Self::from_ring_letter(self.ring(), (self.letter() + 3) % 4)
    }
    pub const fn opposite(self) -> Self {
        Cell(self.0 ^ 1)
    }
    /// Unit normal vector.
    pub fn vector(self) -> [i8; 4] {
        let mut v = [0; 4];
        v[self.axis() as usize] = if self.is_negative() { -1 } else { 1 };
        v
    }

    pub fn parse(s: &str) -> Option<Self> {
        let mut chars = s.chars();
        let (r, l) = (chars.next()?, chars.next()?);
        if chars.next().is_some() {
            return None;
        }
        let ring = RING_CHARS.iter().position(|&c| c == r)?;
        let letter = LETTERS.iter().position(|&c| c == l)?;
        Some(Self::from_ring_letter(ring as u8, letter as u8))
    }
}

impl fmt::Display for Cell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}",
            RING_CHARS[self.ring() as usize],
            LETTERS[self.letter() as usize]
        )
    }
}

/// A signed permutation matrix: basis vector `e_i` maps to `±e_{axis[i]}`,
/// negative when bit `i` of `neg` is set.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Rot {
    axis: [u8; 4],
    neg: u8,
}

impl Rot {
    pub const IDENT: Rot = Rot {
        axis: [0, 1, 2, 3],
        neg: 0,
    };

    /// Builds a rotation from the images of the basis vectors, each given as
    /// `(axis, negative)`. Panics if that is not a permutation.
    pub fn from_images(images: [(u8, bool); 4]) -> Self {
        let mut seen = 0u8;
        let mut r = Rot {
            axis: [0; 4],
            neg: 0,
        };
        for (i, (a, n)) in images.into_iter().enumerate() {
            assert!(a < 4 && seen & (1 << a) == 0, "not a signed permutation");
            seen |= 1 << a;
            r.axis[i] = a;
            r.neg |= (n as u8) << i;
        }
        r
    }

    /// Image of basis vector `e_i` as `(axis, negative)`.
    pub fn image(self, i: u8) -> (u8, bool) {
        (self.axis[i as usize], self.neg >> i & 1 == 1)
    }

    pub fn apply(self, v: [i8; 4]) -> [i8; 4] {
        let mut out = [0; 4];
        for (i, x) in v.into_iter().enumerate() {
            let (a, n) = self.image(i as u8);
            out[a as usize] = if n { -x } else { x };
        }
        out
    }

    pub fn apply_cell(self, c: Cell) -> Cell {
        let (a, n) = self.image(c.axis());
        Cell::from_axis_sign(a, n != c.is_negative())
    }

    /// `self * other`: apply `other` first, then `self`.
    pub fn then_after(self, other: Rot) -> Rot {
        let mut images = [(0, false); 4];
        for i in 0..4 {
            let (a, n) = other.image(i);
            let (b, m) = self.image(a);
            images[i as usize] = (b, n != m);
        }
        Rot::from_images(images)
    }

    pub fn inverse(self) -> Rot {
        let mut images = [(0, false); 4];
        for i in 0..4 {
            let (a, n) = self.image(i);
            images[a as usize] = (i, n);
        }
        Rot::from_images(images)
    }

    pub fn det(self) -> i8 {
        let mut sign = if self.neg.count_ones().is_multiple_of(2) {
            1
        } else {
            -1
        };
        let mut p = self.axis;
        for i in 0..4 {
            while p[i] as usize != i {
                let j = p[i] as usize;
                p.swap(i, j);
                sign = -sign;
            }
        }
        sign
    }

    /// Whether this maps each ring's plane to itself (never swaps ε and η).
    pub fn preserves_rings(self) -> bool {
        (0..4).all(|i| self.image(i).0 >> 1 == i >> 1)
    }
}
