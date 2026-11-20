//! Searches structured algs `(A r)^n` (a short trigger `A` of negative-cell
//! twists, a whole-puzzle rotation `r`) whose result is a phase 2 antipode of
//! some oll target.

use rayon::prelude::*;

use four2four2::coord::{self, Coord, P1, P2, PHASE1_MOVES, PHASE2_MOVES, SlotMove};
use four2four2::geom::{Cell, Rot};
use four2four2::solver::target_symmetries;
use four2four2::state::State;
use four2four2::table;
use four2four2::twist::{Twist, hsc_stm};

type Perm = [u8; 64];

fn bfs<C: Coord>(name: &str, size: usize, moves: &[SlotMove]) -> Vec<u8> {
    table::bfs(name, size, C::solved().index(), |i, f| {
        let c = C::from_index(i);
        for m in moves {
            f(c.apply(m).index());
        }
    })
}

fn slot_coords(p: usize) -> [i8; 4] {
    coord::coords_of(p)
}

/// Gather perm of a twist: `new[x] = old[f[x]]`.
fn twist_perm(t: Twist) -> Perm {
    let (cell, rot) = (t.cell(), t.rot());
    let mut f: Perm = std::array::from_fn(|x| x as u8);
    for p in 0..16 {
        if (p >> cell.axis() & 1 == 1) != cell.is_negative() {
            continue;
        }
        let q = coord::slot_of(rot.apply(slot_coords(p)));
        for i in 0..4u8 {
            let j = rot.image(i).0;
            f[q * 4 + j as usize] = (p * 4 + i as usize) as u8;
        }
    }
    f
}

fn rot_perm(r: Rot) -> Perm {
    let mut f = [0; 64];
    for p in 0..16 {
        let q = coord::slot_of(r.apply(slot_coords(p)));
        for i in 0..4u8 {
            f[q * 4 + r.image(i).0 as usize] = (p * 4 + i as usize) as u8;
        }
    }
    f
}

/// `f` then `g`.
fn then(f: &Perm, g: &Perm) -> Perm {
    std::array::from_fn(|x| f[g[x] as usize])
}

fn state_of(f: &Perm) -> State {
    let solved = State::solved();
    let cells: [Cell; 64] =
        std::array::from_fn(|x| solved.sticker(f[x] as usize / 4, (f[x] % 4) as u8));
    // State is a single [Cell; 64] field
    unsafe { std::mem::transmute::<[Cell; 64], State>(cells) }
}

/// The phase 2 distance of the target `s` is in, if any.
fn p2_dist(t2: &[u8], s: &State) -> Option<u8> {
    let s = s.rotated(coord::normalizing_rotation(s));
    for r in target_symmetries() {
        let u = s.conjugated(r);
        if P1::from_state(&u) == P1::solved() {
            return Some(t2[P2::from_state(&u).index()]);
        }
    }
    None
}

fn rotations() -> Vec<Rot> {
    let mut out = vec![];
    for a in itertools_perms() {
        for neg in 0..16u8 {
            let r = Rot::from_images(std::array::from_fn(|i| (a[i], neg >> i & 1 == 1)));
            let rings_ok = (r.image(0).0 >> 1 == r.image(1).0 >> 1)
                && (r.image(2).0 >> 1 == r.image(3).0 >> 1);
            if rings_ok && r.det() == 1 {
                out.push(r);
            }
        }
    }
    out
}

fn itertools_perms() -> Vec<[u8; 4]> {
    let mut out = vec![];
    for a in 0..4u8 {
        for b in 0..4u8 {
            for c in 0..4u8 {
                for d in 0..4u8 {
                    if [a, b, c, d]
                        .iter()
                        .collect::<std::collections::HashSet<_>>()
                        .len()
                        == 4
                    {
                        out.push([a, b, c, d]);
                    }
                }
            }
        }
    }
    out
}

fn rot_name(r: Rot) -> String {
    let ax = ['x', 'y', 'z', 'w'];
    (0..4u8)
        .map(|i| {
            let (a, n) = r.image(i);
            format!("{}{}", if n { "-" } else { "" }, ax[a as usize])
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// `(a r)^n` as plain twists in the starting frame (the puzzle ends rotated by `r^n`).
fn expand(a: &[Twist], r: Rot, n: usize) -> Vec<Twist> {
    let mut out = vec![];
    let mut cum = Rot::IDENT;
    for _ in 0..n {
        for &t in a {
            out.push(t.conjugated(cum.inverse()));
        }
        cum = r.then_after(cum);
    }
    out
}

fn names(ts: &[Twist]) -> String {
    ts.iter()
        .map(|t| t.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

fn main() {
    let max_len: usize = std::env::args().nth(1).map_or(4, |s| s.parse().unwrap());
    let max_stm: usize = std::env::args().nth(2).map_or(40, |s| s.parse().unwrap());
    let t2 = bfs::<P2>("phase 2", coord::P2_SIZE, &PHASE2_MOVES);
    let moves: Vec<Twist> = PHASE1_MOVES.iter().map(|m| m.twist).collect();
    let mperm: Vec<Perm> = moves.iter().map(|&t| twist_perm(t)).collect();
    let rots = rotations();
    {
        let known = four2four2::twist::parse_hsc_twists("ηD_ηA ηD_ηA εD_ηA ηD_ηC εC_ηA ηD_ηA ηD_ηA ηC_ηB εC_ηA ηD_ηC εD_ηA ηD_ηC εD_ηA ηD_ηA ηC_εA_εB εC_ηA ηD_ηC εD_ηA ηC_ηB εD_ηA ηD_ηA ηD_ηA εD_ηA εC_ηA").unwrap();
        let mut s = State::solved();
        s.apply_all(&known);
        let mut f: Perm = std::array::from_fn(|x| x as u8);
        for &t in &known {
            f = then(&f, &twist_perm(t));
        }
        eprintln!(
            "check: state {:?} perm {:?} same {}",
            p2_dist(&t2, &s),
            p2_dist(&t2, &state_of(&f)),
            state_of(&f) == s
        );
        let r = rots[5];
        let g = then(&f, &rot_perm(r));
        eprintln!("rot same {}", state_of(&g) == s.rotated(r));
    }
    eprintln!("{} rotations", rots.len());
    let rperm: Vec<Perm> = rots.iter().map(|&r| rot_perm(r)).collect();

    // canonical triggers: no two adjacent twists on one cell
    let mut triggers: Vec<Vec<usize>> = vec![vec![]];
    let mut all = vec![];
    for _ in 0..max_len {
        let mut next = vec![];
        for tr in &triggers {
            for j in 0..moves.len() {
                if tr
                    .last()
                    .is_some_and(|&l| moves[l].cell() == moves[j].cell())
                {
                    continue;
                }
                let mut t = tr.clone();
                t.push(j);
                next.push(t);
            }
        }
        all.extend(next.iter().cloned());
        triggers = next;
    }
    eprintln!("{} triggers", all.len());

    // (stm, trigger len, n, trigger, rot)
    let mut found: Vec<(usize, usize, usize, Vec<usize>, usize)> = all
        .par_iter()
        .flat_map_iter(|tr| {
            let mut a: Perm = std::array::from_fn(|x| x as u8);
            for &j in tr {
                a = then(&a, &mperm[j]);
            }
            let mut out = vec![];
            for (ri, rp) in rperm.iter().enumerate() {
                let step = then(&a, rp);
                let mut cur = step;
                for n in 1..=max_stm / tr.len() {
                    if n > 1 {
                        cur = then(&cur, &step);
                    }
                    if n * tr.len() < 21 || n * tr.len() > max_stm {
                        continue;
                    }
                    if p2_dist(&t2, &state_of(&cur)) == Some(21) {
                        out.push((n * tr.len(), tr.len(), n, tr.clone(), ri));
                    }
                }
            }
            out
        })
        .collect();
    found.sort();
    eprintln!("{} hits", found.len());
    let mut hist = std::collections::BTreeMap::new();
    for f in &found {
        *hist.entry((f.0, f.1, f.2)).or_insert(0) += 1;
    }
    println!("(twists, trigger len, n): hits = {hist:?}");
    let mut shown = std::collections::BTreeMap::new();
    for (stm, len, n, tr, ri) in found.iter() {
        let c = shown.entry((*len, *n)).or_insert(0);
        *c += 1;
        if *c > 6 {
            continue;
        }
        let a: Vec<Twist> = tr.iter().map(|&j| moves[j]).collect();
        let r = rots[*ri];
        let ex = expand(&a, r, *n);
        // verify the expansion
        let mut s = State::solved();
        s.apply_all(&ex);
        assert_eq!(p2_dist(&t2, &s), Some(21));
        println!(
            "{stm} twists, hsc stm {}: ({} | rot {}){n}",
            hsc_stm(&ex),
            names(&a),
            rot_name(r)
        );
        if *c == 1 {
            println!("  expanded: {}", names(&ex));
            std::fs::write(
                format!("/tmp/antipode-p2-{stm}.hsc"),
                four2four2::hsc::write(&ex, &[]),
            )
            .unwrap();
        }
    }
}
