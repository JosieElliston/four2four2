//! Replays real Hyperspeedcube logs. `ref/` is gitignored; these tests skip
//! (with a message) when the logs aren't present.

use std::path::{Path, PathBuf};

use four2four2::{hsc, state::State, twist};

fn ref_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("ref")
}

fn logs() -> Vec<PathBuf> {
    let dir = ref_dir().join("ft_polygonal_duoprism~4,4,2,2");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        eprintln!("skipping: {} not found", dir.display());
        return vec![];
    };
    let mut v: Vec<PathBuf> = entries
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "hsc"))
        .collect();
    v.sort();
    v
}

/// The `stmN` HSC puts in log filenames.
fn filename_stm(p: &Path) -> Option<usize> {
    let name = p.file_stem()?.to_str()?;
    name.rsplit_once("_stm")?.1.parse().ok()
}

#[test]
fn hsc_logs_replay_to_solved() {
    for path in logs() {
        let solve = hsc::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let name = path.display();
        assert_eq!(solve.scramble.len(), 1000, "{name}");

        let mut state = State::solved();
        state.apply_all(&solve.scramble);
        assert!(!state.is_solved(), "{name}: scramble is solved");
        state.apply_all(&solve.solution);
        assert!(state.is_solved(), "{name}: not solved");
        assert_eq!(
            Some(twist::hsc_stm(&solve.solution)),
            filename_stm(&path),
            "{name}"
        );
    }
}

#[test]
fn rewritten_logs_still_solve() {
    for path in logs() {
        let solve = hsc::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let back = hsc::parse(&hsc::write(&solve.scramble, &solve.solution)).unwrap();
        assert_eq!(back.scramble, solve.scramble);
        assert_eq!(back.solution, solve.solution);
    }
}

#[test]
fn min_log_parses() {
    let path = ref_dir().join("min.hsc");
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!("skipping: {} not found", path.display());
        return;
    };
    let solve = hsc::parse(&text).unwrap();
    let mut state = State::solved();
    state.apply_all(&solve.scramble);
    state.apply_all(&solve.solution);
    assert!(state.is_solved());
}
