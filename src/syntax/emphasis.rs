use std::cell::Cell;

use memchr::memchr_iter;
use nom::{combinator::map, IResult, Slice};

use super::{
    combinator::{node, token, GreenElement},
    input::Input,
    object::standard_object_nodes,
    SyntaxKind::*,
};

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "debug", skip(input), fields(input = input.s))
)]
pub fn bold_node(input: Input) -> IResult<Input, GreenElement, ()> {
    let mut parser = map(emphasis(b'*'), |contents| {
        let mut children = vec![token(STAR, "*")];
        children.extend(standard_object_nodes(contents));
        children.push(token(STAR, "*"));
        node(BOLD, children)
    });
    crate::lossless_parser!(parser, input)
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "debug", skip(input), fields(input = input.s))
)]
pub fn code_node(input: Input) -> IResult<Input, GreenElement, ()> {
    let mut parser = map(emphasis(b'~'), |contents| {
        node(
            CODE,
            [token(TILDE, "~"), contents.text_token(), token(TILDE, "~")],
        )
    });
    crate::lossless_parser!(parser, input)
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "debug", skip(input), fields(input = input.s))
)]
pub fn strike_node(input: Input) -> IResult<Input, GreenElement, ()> {
    let mut parser = map(emphasis(b'+'), |contents| {
        let mut children = vec![token(PLUS, "+")];
        children.extend(standard_object_nodes(contents));
        children.push(token(PLUS, "+"));
        node(STRIKE, children)
    });
    crate::lossless_parser!(parser, input)
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "debug", skip(input), fields(input = input.s))
)]
pub fn verbatim_node(input: Input) -> IResult<Input, GreenElement, ()> {
    let mut parser = map(emphasis(b'='), |contents| {
        node(
            VERBATIM,
            [token(EQUAL, "="), contents.text_token(), token(EQUAL, "=")],
        )
    });
    crate::lossless_parser!(parser, input)
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "debug", skip(input), fields(input = input.s))
)]
pub fn underline_node(input: Input) -> IResult<Input, GreenElement, ()> {
    let mut parser = map(emphasis(b'_'), |contents| {
        let mut children = vec![token(UNDERSCORE, "_")];
        children.extend(standard_object_nodes(contents));
        children.push(token(UNDERSCORE, "_"));
        node(UNDERLINE, children)
    });
    crate::lossless_parser!(parser, input)
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "debug", skip(input), fields(input = input.s))
)]
pub fn italic_node(input: Input) -> IResult<Input, GreenElement, ()> {
    let mut parser = map(emphasis(b'/'), |contents| {
        let mut children = vec![token(SLASH, "/")];
        children.extend(standard_object_nodes(contents));
        children.push(token(SLASH, "/"));
        node(ITALIC, children)
    });
    crate::lossless_parser!(parser, input)
}

fn emphasis(marker: u8) -> impl Fn(Input) -> IResult<Input, Input, ()> {
    move |input: Input| {
        let bytes = input.as_bytes();

        if bytes.len() < 3 || bytes[0] != marker || input.s[1..].starts_with(is_org_space) {
            return Err(nom::Err::Error(()));
        }

        let closing = CLOSINGS.with(|memo| {
            let end = input.s.as_ptr() as usize + input.s.len();
            let run = memo.get();
            let mut closings = match run.flatten() {
                Some(closings) => {
                    assert_eq!(
                        closings.end, end,
                        "an emphasis starts inside the run of objects it is parsed in"
                    );
                    closings
                }
                None => Closings::new(end),
            };
            let closing = closings.closing(marker, input);
            if run.is_some() {
                memo.set(Some(Some(closings)));
            }
            closing
        });
        match closing {
            Some(idx) => Ok((input.slice(idx + 1..), input.slice(1..idx))),
            None => Err(nom::Err::Error(())),
        }
    }
}

const MARKERS: [u8; 6] = *b"*/_=~+";

thread_local! {
    static CLOSINGS: Cell<Option<Option<Closings>>> = const { Cell::new(None) };
}

/// Held while one run of objects is parsed. The run tries its opening
/// markers from left to right, so each closing-marker scan and each newline
/// search continues where the previous one stopped.
pub struct ClosingScan(Option<Option<Closings>>);

impl ClosingScan {
    pub fn enter() -> ClosingScan {
        ClosingScan(CLOSINGS.with(|memo| memo.replace(Some(None))))
    }
}

impl Drop for ClosingScan {
    fn drop(&mut self) {
        CLOSINGS.with(|memo| memo.set(self.0));
    }
}

/// Addresses in the text of one run of objects, which ends at `end`.
#[derive(Clone, Copy)]
struct Closings {
    end: usize,
    /// Per marker of `MARKERS`: `(from, closing)`, the first valid closing
    /// marker at or after `from`.
    found: [Option<(usize, Option<usize>)>; 6],
    /// `(from, [first, second])`: the first two newlines at or after `from`.
    newlines: Option<(usize, [Option<usize>; 2])>,
}

impl Closings {
    fn new(end: usize) -> Closings {
        Closings {
            end,
            found: [None; 6],
            newlines: None,
        }
    }

    /// The offset of the marker that closes the emphasis `input` opens:
    /// the first valid closing marker after at least one character, unless
    /// two newlines come before it.
    fn closing(&mut self, marker: u8, input: Input) -> Option<usize> {
        let start = input.s.as_ptr() as usize;
        let slot = MARKERS.iter().position(|&m| m == marker).unwrap();
        let from = start + 2;
        let closing = match self.found[slot] {
            Some((scanned, closing)) if scanned <= from && closing.is_none_or(|c| c >= from) => {
                closing
            }
            _ => {
                let closing = memchr_iter(marker, &input.as_bytes()[2..])
                    .map(|idx| idx + 2)
                    .find(|&idx| validate_marker(idx, input))
                    .map(|idx| start + idx);
                self.found[slot] = Some((from, closing));
                closing
            }
        }?;
        let second_newline = self.newlines_from(start, input)[1];
        second_newline
            .is_none_or(|nl| nl > closing)
            .then(|| closing - start)
    }

    fn newlines_from(&mut self, start: usize, input: Input) -> [Option<usize>; 2] {
        let next_newline = |after: usize| {
            memchr::memchr(b'\n', &input.as_bytes()[after - start..]).map(|idx| after + idx)
        };
        let from_start = || {
            let first = next_newline(start);
            [first, first.and_then(|nl| next_newline(nl + 1))]
        };
        let newlines = match self.newlines {
            Some((from, newlines)) if from <= start => match newlines {
                [Some(first), _] if first >= start => newlines,
                [Some(_), Some(second)] if second >= start => {
                    [Some(second), next_newline(second + 1)]
                }
                [Some(_), Some(_)] => from_start(),
                _ => [None, None],
            },
            _ => from_start(),
        };
        self.newlines = Some((start, newlines));
        newlines
    }
}

fn validate_marker(pos: usize, text: Input) -> bool {
    if text.s[..pos].ends_with(is_org_space) {
        false
    } else if let Some(post) = text.s[pos + 1..].chars().next() {
        is_org_space(post)
            || matches!(
                post,
                '-' | '.' | ',' | ';' | ':' | '!' | '?' | '\'' | '"' | ')' | '}' | '\\' | '['
            )
    } else {
        true
    }
}

/// `[[:space:]]` under org-mode's syntax table.
pub fn is_org_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\x0c' | '\r' | ' ' | '\u{a0}' | '\u{2000}'..='\u{200b}' | '\u{202f}' | '\u{205f}' | '\u{3000}'
    )
}

/// org-element's opening border: the start of the contents or one of
/// `[[:space:]] - ( ' " {` before the marker.
pub fn verify_pre(before: &str) -> bool {
    before.chars().next_back().map_or(true, |c| {
        is_org_space(c) || matches!(c, '-' | '(' | '\'' | '"' | '{')
    })
}

#[test]
fn parse() {
    use crate::{ast::Bold, tests::to_ast, ParseConfig};

    let to_bold = to_ast::<Bold>(bold_node);

    insta::assert_debug_snapshot!(
        to_bold("*bold*").syntax,
        @r###"
    BOLD@0..6
      STAR@0..1 "*"
      TEXT@1..5 "bold"
      STAR@5..6 "*"
    "###
    );

    insta::assert_debug_snapshot!(
        to_bold("*bo*ld*").syntax,
        @r###"
    BOLD@0..7
      STAR@0..1 "*"
      TEXT@1..6 "bo*ld"
      STAR@6..7 "*"
    "###
    );

    insta::assert_debug_snapshot!(
        to_bold("*bo\nld*").syntax,
        @r###"
    BOLD@0..7
      STAR@0..1 "*"
      TEXT@1..6 "bo\nld"
      STAR@6..7 "*"
    "###
    );

    let config = &ParseConfig::default();

    assert!(bold_node(("*bold*a", config).into()).is_err());
    assert!(bold_node(("*bold *", config).into()).is_err());
    assert!(bold_node(("* bold*", config).into()).is_err());
    assert!(bold_node(("*b\nol\nd*", config).into()).is_err());
    assert!(italic_node(("*bold*", config).into()).is_err());
}

#[test]
fn an_emphasis_border_is_read_as_org_reads_it() {
    use crate::{rowan::ast::AstNode, Org};

    // `emacs -Q` 30.2, org 9.7.11: each line and the underline org reads in it.
    let cases = [
        ("_\u{a0}a_", None),
        ("_a\u{a0}_", None),
        ("_\u{3000}a_", None),
        ("_a\u{2003}_", None),
        ("_\u{1680}a_", Some("_\u{1680}a_")),
        ("_a_\u{a0}x", Some("_a_")),
        ("_a_\u{3000}x", Some("_a_")),
        ("_a_\u{1680}", None),
        ("_a_\u{b}x", None),
        ("_a_\"", Some("_a_")),
        ("_a_\\", Some("_a_")),
    ];
    let wrong: Vec<String> = cases
        .iter()
        .filter_map(|&(text, org)| {
            let ours = Org::parse(text)
                .document()
                .syntax()
                .descendants()
                .find(|n| n.kind() == UNDERLINE)
                .map(|n| n.to_string());
            (ours.as_deref() != org).then(|| format!("{text:?}: org {org:?}, orgize {ours:?}"))
        })
        .collect();
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
