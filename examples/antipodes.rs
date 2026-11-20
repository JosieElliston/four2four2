//! Counts each phase's antipodes (positions at the table's maximum distance)
//! and ranks them by symmetry.

use rayon::prelude::*;

use four2four2::coord::{self, Coord, P1, P2, PHASE1_MOVES, PHASE2_MOVES, SlotMove};
use four2four2::geom::Rot;
use four2four2::state::State;
use four2four2::table;
use four2four2::twist::Twist;

fn bfs<C: Coord>(name: &str, size: usize, moves: &[SlotMove]) -> Vec<u8> {
    table::bfs(name, size, C::solved().index(), |i, f| {
        let c = C::from_index(i);
        for m in moves {
            f(c.apply(m).index());
        }
    })
}

/// Moves from `c` down to distance 0.
fn path<C: Coord>(t: &[u8], moves: &[SlotMove], mut c: C) -> Vec<Twist> {
    let mut out = vec![];
    while t[c.index()] != 0 {
        let d = t[c.index()];
        let m = moves
            .iter()
            .find(|m| t[c.apply(m).index()] + 1 == d)
            .unwrap();
        out.push(m.twist);
        c = c.apply(m);
    }
    out
}

/// Puzzle symmetries (rotations and mirrors that keep the ring structure)
/// that map the z axis to itself, so they keep the phase targets.
fn syms() -> Vec<Rot> {
    let mut out = vec![];
    for perm in [[0, 1], [1, 0]] {
        for zw in [[2, 3]] {
            for neg in 0..16u8 {
                let axes = [perm[0], perm[1], zw[0], zw[1]];
                out.push(Rot::from_images(std::array::from_fn(|i| {
                    (axes[i], neg >> i & 1 == 1)
                })));
            }
        }
    }
    out
}

fn normalized(s: &State) -> State {
    s.rotated(coord::normalizing_rotation(s))
}

fn state_of(moves_to_solve: &[Twist]) -> State {
    let mut s = State::solved();
    for t in moves_to_solve.iter().rev() {
        s.apply(t.inverse());
    }
    s
}

fn names(ts: &[Twist]) -> String {
    ts.iter()
        .map(|t| t.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

fn cycle_type(s: &State) -> Vec<usize> {
    let p: Vec<usize> = (0..16).map(|q| coord::piece_at(s, q)).collect();
    let mut seen = [false; 16];
    let mut out = vec![];
    for i in 0..16 {
        let mut n = 0;
        let mut j = i;
        while !seen[j] {
            seen[j] = true;
            j = p[j];
            n += 1;
        }
        if n > 1 {
            out.push(n);
        }
    }
    out.sort_unstable_by(|a, b| b.cmp(a));
    out
}

fn report<C: Coord + Send + Sync + PartialEq>(
    name: &str,
    t: &[u8],
    moves: &[SlotMove],
    key: impl Fn(&State) -> C + Sync,
) {
    let max = *t.iter().max().unwrap();
    let anti: Vec<usize> = (0..t.len()).filter(|&i| t[i] == max).collect();
    println!("{name}: {} antipodes at distance {max}", anti.len());
    let syms = syms();
    // (stabilizer, exact involution, inverse is a conjugate, cycle type, index)
    let mut scored: Vec<(usize, bool, bool, Vec<usize>, usize)> = anti
        .par_iter()
        .map(|&i| {
            let c = C::from_index(i);
            let p = path(t, moves, c);
            let s = state_of(&p);
            assert!(key(&s) == c);
            let stab = syms
                .iter()
                .filter(|&&r| key(&normalized(&s.conjugated(r))) == c)
                .count();
            let mut inv = State::solved();
            inv.apply_all(&p);
            let inv = normalized(&inv);
            let invol = key(&inv) == c;
            let conj_inv = syms
                .iter()
                .any(|&r| key(&normalized(&inv.conjugated(r))) == c);
            (stab, invol, conj_inv, cycle_type(&s), i)
        })
        .collect();
    let orbits: f64 = scored.iter().map(|x| x.0 as f64 / syms.len() as f64).sum();
    println!(
        "  {orbits} up to the {} z-preserving symmetries",
        syms.len()
    );
    scored.sort_by(|a, b| b.cmp(a));
    let mut hist = std::collections::BTreeMap::new();
    for (s, inv, cinv, ct, _) in &scored {
        *hist.entry((*s, *inv, *cinv, ct.clone())).or_insert(0) += 1;
    }
    println!(
        "  (stabilizer, involution, inverse ~ conjugate, cycle type of a representative): count"
    );
    for (k, v) in hist.iter().rev() {
        println!("  {k:?}: {v}");
    }
    for (s, inv, cinv, ct, i) in scored.iter().take(4) {
        let p = path(t, moves, C::from_index(*i));
        let scramble: Vec<Twist> = p.iter().rev().map(|t| t.inverse()).collect();
        println!(
            "  stab {s} invol {inv} conj-inv {cinv} cycles {ct:?} idx {i} stm {}: {}",
            four2four2::twist::hsc_stm(&scramble),
            names(&scramble)
        );
        let path = format!("/tmp/antipode-{}-{i}.hsc", name.replace(' ', ""));
        std::fs::write(&path, four2four2::hsc::write(&scramble, &[])).unwrap();
    }
}

/// Every optimal path from `c` down to 0.
fn all_paths<C: Coord>(
    t: &[u8],
    moves: &[SlotMove],
    c: C,
    cur: &mut Vec<Twist>,
    out: &mut Vec<Vec<Twist>>,
) {
    let d = t[c.index()];
    if d == 0 {
        out.push(cur.clone());
        return;
    }
    for m in moves {
        let n = c.apply(m);
        if t[n.index()] + 1 == d {
            cur.push(m.twist);
            all_paths(t, moves, n, cur, out);
            cur.pop();
        }
    }
}

fn state_stab(s: &State) -> usize {
    syms()
        .iter()
        .filter(|&&r| normalized(&s.conjugated(r)) == *s)
        .count()
}

fn is_invol(s: &State) -> bool {
    let p: Vec<usize> = (0..16).map(|q| coord::piece_at(s, q)).collect();
    // as a state: applying it twice is solved
    let mut t = s.clone();
    // s = solved * X; build X X by composing sticker maps isn't exposed, so
    // compare with the inverse via the permutation and orientation: an
    // involution's permutation is an involution and it equals its inverse.
    let _ = &mut t;
    (0..16).all(|q| p[p[q]] == q)
}

fn extra() {
    let syms = syms();
    // phase 2: orbits of antipodes, how many members are involutions
    let t2 = bfs::<P2>("phase 2", coord::P2_SIZE, &PHASE2_MOVES);
    let max = *t2.iter().max().unwrap();
    let anti: Vec<usize> = (0..t2.len()).filter(|&i| t2[i] == max).collect();
    let info: Vec<(usize, usize, bool, Vec<usize>, Vec<Twist>)> = anti
        .par_iter()
        .map(|&i| {
            let p = path(&t2, &PHASE2_MOVES, P2::from_index(i));
            let s = state_of(&p);
            let mut inv = State::solved();
            inv.apply_all(&p);
            let canon = syms
                .iter()
                .map(|&r| P2::from_state(&normalized(&s.conjugated(r))).index())
                .min()
                .unwrap();
            let scramble: Vec<Twist> = p.iter().rev().map(|t| t.inverse()).collect();
            (canon, i, normalized(&inv) == s, cycle_type(&s), scramble)
        })
        .collect();
    let mut orbits: std::collections::BTreeMap<
        usize,
        Vec<&(usize, usize, bool, Vec<usize>, Vec<Twist>)>,
    > = Default::default();
    for x in &info {
        orbits.entry(x.0).or_default().push(x);
    }
    let mut ranked: Vec<_> = orbits
        .values()
        .map(|v| (v.iter().filter(|x| x.2).count(), v))
        .collect();
    ranked.sort_by(|a, b| b.0.cmp(&a.0));
    let mut h = std::collections::BTreeMap::new();
    for (n, _) in &ranked {
        *h.entry(*n).or_insert(0) += 1;
    }
    println!("phase 2 orbits by involution members (of 32): {h:?}");
    for (n, v) in ranked.iter().take(3) {
        println!("orbit with {n} involution frames:");
        for x in v.iter().filter(|x| x.2) {
            println!("  idx {} cycles {:?}: {}", x.1, x.3, names(&x.4));
        }
    }
    let (_, v) = &ranked[0];
    let best = v.iter().find(|x| x.2).unwrap();
    std::fs::write(
        "/tmp/antipode-phase2-best.hsc",
        four2four2::hsc::write(&best.4, &[]),
    )
    .unwrap();
    drop(t2);

    // phase 1: for the most symmetric cosets, look for a symmetric representative
    let t1 = bfs::<P1>("phase 1", coord::P1_SIZE, &PHASE1_MOVES);
    let anti: Vec<usize> = (0..t1.len()).filter(|&i| t1[i] == 10).collect();
    let stab4: Vec<usize> = anti
        .par_iter()
        .copied()
        .filter(|&i| {
            let c = P1::from_index(i);
            let s = state_of(&path(&t1, &PHASE1_MOVES, c));
            syms.iter()
                .filter(|&&r| P1::from_state(&normalized(&s.conjugated(r))) == c)
                .count()
                == 4
        })
        .collect();
    println!("phase 1: {} stab-4 cosets", stab4.len());
    let res: Vec<(usize, usize, usize, Vec<Twist>)> = stab4
        .par_iter()
        .map(|&i| {
            let mut out = vec![];
            all_paths(&t1, &PHASE1_MOVES, P1::from_index(i), &mut vec![], &mut out);
            let n = out.len();
            let (st, p) = out
                .into_iter()
                .map(|p| (state_stab(&state_of(&p)), p))
                .max_by_key(|x| x.0)
                .unwrap();
            (st, n, i, p.iter().rev().map(|t| t.inverse()).collect())
        })
        .collect();
    let mut h = std::collections::BTreeMap::new();
    for r in &res {
        *h.entry(r.0).or_insert(0) += 1;
    }
    println!("phase 1 stab-4 cosets by best representative's own stabilizer: {h:?}");
    let best = res.iter().max_by_key(|r| r.0).unwrap();
    for r in res.iter().filter(|r| r.0 == best.0).take(8) {
        println!(
            "  coset {} ({} optimal paths): rep stab {}: {}",
            r.2,
            r.1,
            r.0,
            names(&r.3)
        );
        let c = P1::from_index(r.2);
        println!("    O-mask {:016b} ori-mask {:016b}", c.o(), c.ori());
    }
    std::fs::write(
        "/tmp/antipode-phase1-best.hsc",
        four2four2::hsc::write(&best.3, &[]),
    )
    .unwrap();
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("extra") {
        extra();
        return;
    }
    if std::env::args().nth(1).as_deref() == Some("check") {
        for m in PHASE2_MOVES.iter().take(12) {
            let mut s = State::solved();
            s.apply(m.twist);
            let c = P2::from_state(&s);
            let stab: Vec<_> = syms()
                .into_iter()
                .filter(|&r| P2::from_state(&normalized(&s.conjugated(r))) == c)
                .collect();
            let p1ok = syms()
                .iter()
                .all(|&r| P1::from_state(&normalized(&s.conjugated(r))) == P1::solved());
            println!("{} stab {} p1ok {p1ok}", m.twist, stab.len());
        }
        let s = State::solved();
        println!(
            "solved stab {}",
            syms()
                .iter()
                .filter(|&&r| P2::from_state(&normalized(&s.conjugated(r))) == P2::solved())
                .count()
        );
        return;
    }
    let t1 = bfs::<P1>("phase 1", coord::P1_SIZE, &PHASE1_MOVES);
    report("phase 1", &t1, &PHASE1_MOVES, P1::from_state);
    drop(t1);
    let t2 = bfs::<P2>("phase 2", coord::P2_SIZE, &PHASE2_MOVES);
    report("phase 2", &t2, &PHASE2_MOVES, P2::from_state);
}
