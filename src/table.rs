//! Complete distance tables, one byte per coordinate, built by breadth-first
//! search and cached on disk.

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
