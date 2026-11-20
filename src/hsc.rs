//! Reading and writing Hyperspeedcube 2 log files (`.hsc`).
//!
//! Only the parts this solver needs are read: the puzzle id, the scramble's
//! literal twist list, and the `twists`/`undo`/`redo` entries of the log. The
//! file is KDL, but HSC writes one node per line, so a line-based reader is
//! enough for logs HSC produces.

use std::fmt::{self, Write as _};

use crate::twist::{ParseTwistError, Twist, parse_hsc_twists};

pub const PUZZLE_ID: &str = "ft_polygonal_duoprism:4,4,2,2";
pub const PUZZLE_VERSION: &str = "1.0.1";

/// The same puzzle in Hyperspeedcube after 2.0.0-zeta.12, where duoprisms are
/// products of polygons. See [`port_to_latest`].
pub const LATEST_PUZZLE_ID: &str = "product@1([ngon_ft_shallow(4,2),ngon_ft_shallow(4,2)])";

#[derive(Clone, Debug, Default)]
pub struct Solve {
    pub puzzle_id: String,
    pub puzzle_version: Option<String>,
    pub scramble: Vec<Twist>,
    /// The twists of the solve after applying `undo`/`redo`, in order.
    pub solution: Vec<Twist>,
    /// The `solved` field, if present.
    pub solved_flag: Option<bool>,
}

#[derive(Debug)]
pub enum Error {
    Twist {
        line: usize,
        err: ParseTwistError,
    },
    /// A scramble given only by seed, which we can't regenerate.
    NoScrambleTwists,
    NoSolve,
    WrongPuzzle(String),
    /// A log file version other than the one 2.0.0-zeta.12 writes.
    UnsupportedVersion(String),
    /// A node every log has, such as `version`, is missing.
    MissingNode(&'static str),
    Syntax {
        line: usize,
        msg: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Twist { line, err } => write!(f, "line {line}: {err}"),
            Error::NoScrambleTwists => write!(f, "scramble has no literal twist list"),
            Error::NoSolve => write!(f, "no `solve` block"),
            Error::WrongPuzzle(id) => write!(f, "puzzle is {id:?}, expected {PUZZLE_ID:?}"),
            Error::UnsupportedVersion(v) => {
                write!(f, "log file version is {v}, expected 2 (2.0.0-zeta.12)")
            }
            Error::MissingNode(name) => write!(f, "no `{name}` node"),
            Error::Syntax { line, msg } => write!(f, "line {line}: {msg}"),
        }
    }
}
impl std::error::Error for Error {}

/// Value of `key=value` or `key="value"` in a KDL node line.
fn prop<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let start = line.find(&format!("{key}="))? + key.len() + 1;
    let rest = &line[start..];
    if let Some(rest) = rest.strip_prefix('"') {
        rest.split('"').next()
    } else {
        rest.split_whitespace().next()
    }
}

/// The content of the first quoted string on the line, or the bare words after
/// the node name.
fn args(line: &str) -> &str {
    match (line.find('"'), line.rfind('"')) {
        (Some(a), Some(b)) if a < b => &line[a + 1..b],
        _ => line.split_once(char::is_whitespace).map_or("", |(_, r)| r),
    }
}

/// Parses the first solve in an HSC2 log.
pub fn parse(text: &str) -> Result<Solve, Error> {
    let mut solve = Solve::default();
    let mut found_solve = false;
    let mut found_scramble_twists = false;
    // block path, e.g. ["solve", "log"]
    let mut stack: Vec<String> = vec![];
    // each `twists` node is one undo step
    let mut done: Vec<Vec<Twist>> = vec![];
    let mut undone: Vec<Vec<Twist>> = vec![];

    for (i, raw) in text.lines().enumerate() {
        let line_no = i + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        if line == "}" {
            if stack.pop().is_none() {
                return Err(Error::Syntax {
                    line: line_no,
                    msg: "unmatched `}`".into(),
                });
            }
            continue;
        }
        let name = line.split_whitespace().next().unwrap();
        let path: Vec<&str> = stack.iter().map(String::as_str).collect();
        let twists =
            |s: &str| parse_hsc_twists(s).map_err(|err| Error::Twist { line: line_no, err });

        match (path.as_slice(), name) {
            ([], "solve") => found_solve = true,
            (["solve"], "puzzle") => {
                solve.puzzle_id = prop(line, "id").unwrap_or_default().to_owned();
                solve.puzzle_version = prop(line, "version").map(str::to_owned);
            }
            (["solve"], "solved") => solve.solved_flag = Some(line.contains("#true")),
            (["solve", "scramble"], "twists") => {
                solve.scramble = twists(args(line))?;
                found_scramble_twists = true;
            }
            (["solve", "log"], "twists") => {
                done.push(twists(args(line))?);
                undone.clear();
            }
            (["solve", "log"], "undo") => {
                if let Some(t) = done.pop() {
                    undone.push(t);
                }
            }
            (["solve", "log"], "redo") => {
                if let Some(t) = undone.pop() {
                    done.push(t);
                }
            }
            _ => {}
        }
        if line.ends_with('{') {
            stack.push(name.to_owned());
        }
        // only the first solve
        if found_solve && stack.is_empty() {
            break;
        }
    }

    if !found_solve {
        return Err(Error::NoSolve);
    }
    if solve.puzzle_id != PUZZLE_ID {
        return Err(Error::WrongPuzzle(solve.puzzle_id));
    }
    if !found_scramble_twists {
        return Err(Error::NoScrambleTwists);
    }
    solve.solution = done.concat();
    Ok(solve)
}

fn names(twists: &[Twist]) -> impl Iterator<Item = String> + '_ {
    twists.iter().flat_map(|t| t.hsc_names())
}

/// Writes a minimal HSC2 log that HSC2 (2.0.0-zeta.12) loads: the scramble as
/// a literal twist list, then the solution, one twist per `twists` node.
pub fn write(scramble: &[Twist], solution: &[Twist]) -> String {
    let mut s = String::new();
    s += "// Hyperspeedcube puzzle log\n";
    s += "version 2\n";
    writeln!(
        s,
        "program name={} version=\"{}\"",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    )
    .unwrap();
    s += "solve {\n";
    writeln!(s, "    puzzle id={PUZZLE_ID} version=\"{PUZZLE_VERSION}\"").unwrap();
    s += "    scramble full version=1 {\n";
    writeln!(
        s,
        "        twists \"{}\"",
        names(scramble).collect::<Vec<_>>().join(" ")
    )
    .unwrap();
    s += "    }\n";
    s += "    log {\n";
    s += "        scramble\n";
    for n in names(solution) {
        writeln!(s, "        twists {n}").unwrap();
    }
    s += "    }\n";
    s += "}\n";
    s
}

/// Converts a 2.0.0-zeta.12 log of this puzzle to the format of later
/// Hyperspeedcube versions: log file version 3, [`LATEST_PUZZLE_ID`], and twist
/// axes renamed from `ε`/`η` to `a`/`b`.
///
/// Everything else, including replay events, is kept as-is. The converted twists
/// were checked to reach the same states in Hyperspeedcube at commit `65fecbd8`
/// (a solved log stays solved), so the axis renaming preserves meaning.
pub fn port_to_latest(text: &str) -> Result<String, Error> {
    let mut saw_version = false;
    let mut saw_puzzle = false;
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let trimmed = line.trim_start();
        let indent = &line[..line.len() - trimmed.len()];
        if let Some(version) = trimmed.strip_prefix("version ") {
            let version = version.trim();
            if version != "2" {
                return Err(Error::UnsupportedVersion(version.to_owned()));
            }
            saw_version = true;
            writeln!(out, "{indent}version 3").unwrap();
        } else if trimmed.starts_with("puzzle ") {
            let id = prop(trimmed, "id").unwrap_or_default();
            if id != PUZZLE_ID {
                return Err(Error::WrongPuzzle(id.to_owned()));
            }
            saw_puzzle = true;
            writeln!(out, "{indent}puzzle id=\"{LATEST_PUZZLE_ID}\" version=\"\"").unwrap();
        } else {
            out += &line.replace('ε', "a").replace('η', "b");
            out += "\n";
        }
    }
    if !saw_version {
        return Err(Error::MissingNode("version"));
    }
    if !saw_puzzle {
        return Err(Error::MissingNode("puzzle"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::State;

    const SAMPLE: &str = r#"// Hyperspeedcube puzzle log
version 2
program name=Hyperspeedcube version="2.0.0-zeta.12"
solve {
    replay #true
    puzzle id=ft_polygonal_duoprism:4,4,2,2 version="1.0.1"
    solved #true
    scramble full version=1 time="t" seed="s" {
        twists "εA_εB ηA_εB"
        drand_round_v1 round=1 signature="x"
    }
    log {
        scramble time="t"
        click time="t" layers=1 target=εB_εA reverse=#true
        twists ηA_εB
        twists ηC_ηD
        undo time="t"
        twists εA_εD
        undo time="t"
        redo time="t"
        twists εA_εB
        undo time="t"
    }
    tsa_signature_v3 "00"
}
"#;

    #[test]
    fn parse_sample() {
        let s = parse(SAMPLE).unwrap();
        assert_eq!(s.puzzle_version.as_deref(), Some("1.0.1"));
        assert_eq!(s.solved_flag, Some(true));
        assert_eq!(s.scramble, parse_hsc_twists("εA_εB ηA_εB").unwrap());
        assert_eq!(s.solution, parse_hsc_twists("ηA_εB εA_εD").unwrap());
        let mut st = State::solved();
        st.apply_all(&s.scramble);
        assert!(!st.is_solved());
        st.apply_all(&s.solution);
        assert!(st.is_solved());
    }

    #[test]
    fn write_round_trips() {
        let s = parse(SAMPLE).unwrap();
        let half = Twist::half(s.scramble[0].cell());
        let mut solution = s.solution.clone();
        solution.push(half);
        let text = write(&s.scramble, &solution);
        let back = parse(&text).unwrap();
        assert_eq!(back.scramble, s.scramble);
        // the half turn comes back as two quarter turns
        assert_eq!(back.solution.len(), solution.len() + 1);
        assert_eq!(
            crate::twist::simplify(&back.solution),
            crate::twist::simplify(&solution)
        );
    }

    #[test]
    fn seed_only_scramble_is_rejected() {
        let text = SAMPLE.replace("        twists \"εA_εB ηA_εB\"\n", "");
        assert!(matches!(parse(&text), Err(Error::NoScrambleTwists)));
    }

    #[test]
    fn port_to_latest_renames_puzzle_and_axes() {
        let ported = port_to_latest(SAMPLE).unwrap();
        assert!(ported.contains("\nversion 3\n"));
        assert!(ported.contains(&format!(
            "    puzzle id=\"{LATEST_PUZZLE_ID}\" version=\"\"\n"
        )));
        assert!(ported.contains("        twists \"aA_aB bA_aB\"\n"));
        assert!(ported.contains("target=aB_aA"));
        assert!(!ported.contains(['ε', 'η']));
        assert_eq!(ported.lines().count(), SAMPLE.lines().count());
    }

    #[test]
    fn port_to_latest_rejects_other_logs() {
        let ported = port_to_latest(SAMPLE).unwrap();
        assert!(matches!(
            port_to_latest(&ported),
            Err(Error::UnsupportedVersion(v)) if v == "3"
        ));
        let other = SAMPLE.replace(PUZZLE_ID, "ft_cube:3");
        assert!(matches!(port_to_latest(&other), Err(Error::WrongPuzzle(_))));
    }
}
