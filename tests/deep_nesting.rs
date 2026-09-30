use std::time::{Duration, Instant};

use orgize::{rowan::ast::AstNode, Org};

fn nested(open: &str, close: &str, depth: usize) -> String {
    format!("{}x{}", open.repeat(depth), close.repeat(depth))
}

fn nested_list(depth: usize) -> String {
    (0..depth)
        .map(|i| format!("{}- x\n", " ".repeat(i)))
        .collect()
}

fn shapes() -> Vec<(&'static str, String)> {
    vec![
        ("bare stars", "*".repeat(100_000)),
        ("bare slashes", "/".repeat(100_000)),
        ("bare pluses", "+".repeat(100_000)),
        ("bare underscores", "_".repeat(100_000)),
        (
            "headline title of stars",
            format!("* {}\n", "*".repeat(100_000)),
        ),
        ("nested bold", nested("*a ", " a*", 3_000)),
        ("nested emphasis", nested("*/_+", "+_/*", 20_000)),
        ("nested superscript", nested("a^{", "}", 20_000)),
        ("nested footnote references", nested("[fn::", "]", 20_000)),
        ("nested link descriptions", nested("[[x][", "]]", 20_000)),
        (
            "nested quote blocks",
            nested("#+begin_quote\n", "#+end_quote\n", 2_000),
        ),
        ("nested drawers", nested(":D:\n", ":END:\n", 2_000)),
        ("nested list", nested_list(2_000)),
        ("open emphasis run", "*a ".repeat(3_000)),
    ]
}

#[test]
fn deep_nesting_parses_losslessly_in_bounded_time() {
    for (name, input) in shapes() {
        let started = Instant::now();
        let org = Org::parse(&input);
        assert_eq!(org.to_org(), input, "{name}: not lossless");
        drop(org);
        let took = started.elapsed();
        assert!(took < Duration::from_secs(60), "{name}: took {took:?}");
    }
}

#[test]
fn nesting_below_the_limit_is_read() {
    let org = Org::parse(format!("{}\n", "*".repeat(21)));
    let depth = org
        .document()
        .syntax()
        .descendants()
        .filter(|n| n.kind() == orgize::SyntaxKind::BOLD)
        .count();
    assert_eq!(depth, 10);
}
