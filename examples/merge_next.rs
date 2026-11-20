//! Merges [`coset_next`](coset_next.rs) slices into an exact distance table.
//!
//! DIST is coset's output for T-1 (exact up to T-1, upper bounds above). Each
//! HITS file is a coset_next output for T. A position whose bound is above T is
//! T if hit, and otherwise at least T+1; it is written as T+1, which is exact
//! only if every position is known to be at most T+1 (step 3 of antipode.md,
//! via `residual`).
//!
//! usage: merge_next DIST T OUT HITS...
use std::collections::BTreeMap;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 5 {
        eprintln!("usage: merge_next DIST T OUT HITS...");
        std::process::exit(2);
    }
    let mut d = std::fs::read(&args[1]).unwrap();
    let t: u8 = args[2].parse().unwrap();
    let mut hit = vec![false; d.len()];
    for p in &args[4..] {
        for c in std::fs::read(p).unwrap().chunks_exact(4) {
            hit[u32::from_le_bytes(c.try_into().unwrap()) as usize] = true;
        }
    }
    for (i, x) in d.iter_mut().enumerate() {
        if *x > t {
            *x = if hit[i] { t } else { t + 1 };
        } else {
            assert!(!hit[i], "position {i} with bound {x} <= {t} was hit");
        }
    }
    let mut h = BTreeMap::new();
    for &x in &d {
        *h.entry(x).or_insert(0usize) += 1;
    }
    for (k, v) in &h {
        println!("{k:2}: {v}");
    }
    std::fs::write(&args[3], &d).unwrap();
}
