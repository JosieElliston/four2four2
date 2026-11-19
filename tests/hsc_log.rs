//! Replays a real Hyperspeedcube 2 (2.0.0-zeta.12) log: a 1000-twist scramble
//! and a 35 STM human solve with clicks, undos and signatures.

use std::path::Path;

use four2four2::{hsc, state::State, twist};

const LOG: &str = include_str!("data/stm35.hsc");

#[test]
fn log_replays_to_solved() {
    let solve = hsc::parse(LOG).unwrap();
    assert_eq!(solve.scramble.len(), 1000);
    assert_eq!(solve.solved_flag, Some(true));

    let mut state = State::solved();
    state.apply_all(&solve.scramble);
    assert!(!state.is_solved());
    state.apply_all(&solve.solution);
    assert!(state.is_solved());
    // HSC named the file `..._stm35.hsc`
    assert_eq!(twist::hsc_stm(&solve.solution), 35);
}

#[test]
fn rewritten_log_round_trips() {
    let solve = hsc::parse(LOG).unwrap();
    let back = hsc::parse(&hsc::write(&solve.scramble, &solve.solution)).unwrap();
    assert_eq!(back.scramble, solve.scramble);
    assert_eq!(back.solution, solve.solution);
}

/// Builds (or loads) the full tables, so it's slow the first time and needs
/// ~420 MB. Run with `cargo test --release -- --ignored`.
#[test]
#[ignore]
fn solver_solves_log_scramble() {
    use four2four2::solver::{Tables, solve};
    let tables = Tables::load_or_build(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tables"));
    let mut state = State::solved();
    state.apply_all(&hsc::parse(LOG).unwrap().scramble);
    let sol = solve(&tables, &state, Default::default());
    state.apply_all(&sol.twists());
    assert!(state.is_solved());
    // each phase is at most its table's maximum depth
    assert!(sol.phase1.len() <= 10 && sol.phase2.len() <= 21);
}
