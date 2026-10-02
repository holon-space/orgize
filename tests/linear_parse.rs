//! The only test of this binary: the instruction count is per process, so no
//! other test may retire instructions while it measures.

use orgize::{rowan::ast::AstNode, Org};

const MARKERS: [char; 6] = ['*', '/', '_', '=', '~', '+'];

/// One character of each class org-element accepts before an emphasis
/// marker. The start of the contents is the remaining class: `*{marker}a*`.
const BORDERS: [&str; 8] = [" ", "\u{a0}", "\u{3000}", "-", "(", "'", "\"", "{"];

fn units() -> Vec<String> {
    let mut units = Vec::new();
    for marker in MARKERS {
        for border in BORDERS {
            units.push(format!("{border}{marker}a"));
        }
        let container = if marker == '*' { '/' } else { '*' };
        units.push(format!("{container}{marker}a{container} "));
    }
    for border in BORDERS {
        units.push(format!("{border}_{{a"));
    }
    units
}

fn contexts(body: &str) -> [(&'static str, String); 4] {
    [
        ("paragraph", body.to_string()),
        ("headline title", format!("* {body}\n")),
        ("table cell", format!("| {body} |\n")),
        ("link description", format!("[[t][{body}]]")),
    ]
}

#[cfg(target_os = "macos")]
fn instructions_retired() -> u64 {
    extern "C" {
        fn proc_pid_rusage(pid: i32, flavor: i32, buffer: *mut u64) -> i32;
    }
    const RUSAGE_INFO_V4: i32 = 4;
    const RI_INSTRUCTIONS: usize = 31;
    let mut info = [0u64; 64];
    let status =
        unsafe { proc_pid_rusage(std::process::id() as i32, RUSAGE_INFO_V4, info.as_mut_ptr()) };
    assert_eq!(
        status,
        0,
        "proc_pid_rusage failed: {}",
        std::io::Error::last_os_error()
    );
    info[RI_INSTRUCTIONS]
}

#[cfg(not(target_os = "macos"))]
fn instructions_retired() -> u64 {
    unreachable!("the test is ignored off macOS")
}

fn parse_cost(text: &str) -> u64 {
    (0..3)
        .map(|_| {
            let start = instructions_retired();
            let nodes = Org::parse(text).document().syntax().descendants().count();
            let cost = instructions_retired() - start;
            std::hint::black_box(nodes);
            cost
        })
        .min()
        .expect("three runs")
}

#[test]
#[cfg_attr(
    not(target_os = "macos"),
    ignore = "counts retired instructions through macOS proc_pid_rusage"
)]
fn an_opening_marker_with_no_closing_marker_parses_in_linear_time() {
    const SIZES: [usize; 3] = [250, 500, 1_000];
    let mut wrong = Vec::new();
    for unit in units() {
        for (context, _) in contexts(&unit) {
            let costs: Vec<u64> = SIZES
                .iter()
                .map(|&n| {
                    let texts = contexts(&unit.repeat(n));
                    let (_, text) = texts.iter().find(|(c, _)| *c == context).unwrap();
                    parse_cost(text)
                })
                .collect();
            // A quadratic term grows by 4 per doubling, a linear one by 2.
            if costs.windows(2).any(|pair| pair[1] * 2 > pair[0] * 5) {
                wrong.push(format!(
                    "{unit:?} in a {context}: {costs:?} instructions for {SIZES:?} repeats"
                ));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "{} shapes do not parse in linear time:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}
