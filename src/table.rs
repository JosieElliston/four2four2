//! Complete distance tables, built by breadth-first search one byte per
//! coordinate, then packed ([`Mod3`], [`Nibbles`]) and cached on disk.

use std::path::Path;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::time::Instant;

use rayon::prelude::*;

pub const UNREACHED: u8 = u8::MAX;

/// Level-by-level BFS from `start`. `neighbors(i, f)` must call `f` on every
/// neighbor of index `i`. Moves must be closed under inverse.
pub fn bfs<N>(name: &str, size: usize, start: usize, neighbors: N) -> Vec<u8>
where
    N: Fn(usize, &mut dyn FnMut(usize)) + Sync,
{
    let t0 = Instant::now();
    let table: Vec<AtomicU8> = (0..size).map(|_| AtomicU8::new(UNREACHED)).collect();
    table[start].store(0, Ordering::Relaxed);
    let mut total = 1;
    for depth in 0..UNREACHED - 1 {
        let found = AtomicUsize::new(0);
        table
            .par_iter()
            .enumerate()
            .with_min_len(1 << 16)
            .for_each(|(i, d)| {
                if d.load(Ordering::Relaxed) != depth {
                    return;
                }
                let mut n = 0;
                neighbors(i, &mut |j| {
                    if table[j]
                        .compare_exchange(
                            UNREACHED,
                            depth + 1,
                            Ordering::Relaxed,
                            Ordering::Relaxed,
                        )
                        .is_ok()
                    {
                        n += 1;
                    }
                });
                found.fetch_add(n, Ordering::Relaxed);
            });
        let found = found.into_inner();
        if found == 0 {
            break;
        }
        total += found;
        eprintln!(
            "{name}: depth {:2}: {found:>11} new, {total:>11} total ({:.1?})",
            depth + 1,
            t0.elapsed()
        );
    }
    table.into_iter().map(AtomicU8::into_inner).collect()
}

/// Loads `path` if it exists with the right size, otherwise builds the table
/// and writes it there.
pub fn load_or_build(path: &Path, size: usize, build: impl FnOnce() -> Vec<u8>) -> Vec<u8> {
    if let Ok(data) = std::fs::read(path)
        && data.len() == size
    {
        return data;
    }
    let data = build();
    assert_eq!(data.len(), size);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = std::fs::write(path, &data) {
        eprintln!("warning: couldn't cache table at {}: {e}", path.display());
    }
    data
}

/// Distances mod 3, 2 bits each, 4 per byte. That's enough when the caller
/// knows a neighbor's exact distance, since moves are invertible: neighbors
/// differ by at most 1, and -1, 0, +1 are distinct mod 3. See
/// [`child_distance`].
pub struct Mod3(Vec<u8>);

impl Mod3 {
    /// Packs a table of exact distances, which must all be reached.
    pub fn pack(d: &[u8]) -> Self {
        assert!(!d.contains(&UNREACHED), "unreached entries can't be packed");
        Mod3(
            d.chunks(4)
                .map(|c| c.iter().enumerate().map(|(j, &x)| (x % 3) << (2 * j)).sum())
                .collect(),
        )
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Mod3(bytes)
    }

    pub fn bytes(&self) -> &[u8] {
        &self.0
    }

    /// Bytes needed for `n` entries.
    pub const fn packed_len(n: usize) -> usize {
        n.div_ceil(4)
    }

    #[inline]
    pub fn get(&self, i: usize) -> u8 {
        self.0[i >> 2] >> (2 * (i & 3)) & 3
    }
}

/// Distances in 4 bits, 2 per byte: below [`Nibbles::EXACT`] exactly, and
/// from there on as `EXACT + d % 3`. A lookup alone tells whether a distance
/// is below `EXACT`; past that, stepping downhill (as with [`Mod3`]) reaches
/// exact values.
pub struct Nibbles(Vec<u8>);

impl Nibbles {
    pub const EXACT: u8 = 13;

    /// Packs a table of exact distances, which must all be reached.
    pub fn pack(d: &[u8]) -> Self {
        assert!(!d.contains(&UNREACHED), "unreached entries can't be packed");
        let code = |x: u8| {
            if x < Self::EXACT {
                x
            } else {
                Self::EXACT + x % 3
            }
        };
        Nibbles(
            d.chunks(2)
                .map(|c| c.iter().enumerate().map(|(j, &x)| code(x) << (4 * j)).sum())
                .collect(),
        )
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Nibbles(bytes)
    }

    pub fn bytes(&self) -> &[u8] {
        &self.0
    }

    /// Bytes needed for `n` entries.
    pub const fn packed_len(n: usize) -> usize {
        n.div_ceil(2)
    }

    #[inline]
    pub fn get(&self, i: usize) -> u8 {
        self.0[i >> 1] >> (4 * (i & 1)) & 15
    }

    /// Whether a neighbor with code `n` is one step closer than a position
    /// with code `c`.
    #[inline]
    pub fn is_downhill(c: u8, n: u8) -> bool {
        if c < Self::EXACT {
            n + 1 == c
        } else {
            // d >= EXACT, so one closer is either EXACT - 1 (when d = EXACT)
            // or one less mod 3
            n == Self::EXACT - 1 || n == Self::EXACT + (c - Self::EXACT + 2) % 3
        }
    }
}

/// Exact distance of a neighbor of a position at distance `parent`, given the
/// neighbor's [`Mod3`] entry `v`.
#[inline]
pub fn child_distance(parent: u8, v: u8) -> u8 {
    child_distances(parent)[v as usize]
}

/// [`child_distance`] for each entry value, as a table, so that decoding
/// loaded entries doesn't branch on them.
#[inline]
pub fn child_distances(parent: u8) -> [u8; 4] {
    // v = parent + delta (mod 3), delta in -1..=1
    std::array::from_fn(|v| (parent + (v as u8 + 4 - parent % 3) % 3).wrapping_sub(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nibbles_round_trip() {
        let d: Vec<u8> = (0..1001).map(|i| (i * 7 % 23) as u8).collect();
        let t = Nibbles::pack(&d);
        assert_eq!(t.bytes().len(), Nibbles::packed_len(d.len()));
        for (i, &x) in d.iter().enumerate() {
            let e = Nibbles::EXACT;
            assert_eq!(t.get(i), if x < e { x } else { e + x % 3 });
        }
        // downhill is recognized exactly between neighboring distances
        for x in 1..30u8 {
            let code = |x: u8| t.get(d.iter().position(|&y| y == x).unwrap());
            for n in x.saturating_sub(1)..=x + 1 {
                if n < 23 && x < 23 {
                    assert_eq!(
                        Nibbles::is_downhill(code(x), code(n)),
                        n + 1 == x,
                        "{x} {n}"
                    );
                }
            }
        }
    }

    #[test]
    fn mod3_round_trips() {
        let d: Vec<u8> = (0..1001).map(|i| (i * 7 % 23) as u8).collect();
        let t = Mod3::pack(&d);
        assert_eq!(t.bytes().len(), Mod3::packed_len(d.len()));
        for (i, &x) in d.iter().enumerate() {
            assert_eq!(t.get(i), x % 3);
            for p in [x.saturating_sub(1), x, x + 1] {
                if p.abs_diff(x) <= 1 {
                    assert_eq!(child_distance(p, t.get(i)), x);
                }
            }
        }
    }
}
