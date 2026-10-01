use std::cell::Cell;

use nom::{
    branch::alt,
    combinator::map,
    sequence::tuple,
    IResult, InputTake,
};

use super::{
    combinator::{
        l_bracket2_token, l_bracket_token, node, r_bracket2_token, r_bracket_token, GreenElement,
    },
    input::Input,
    object::link_description_object_nodes,
    SyntaxKind::*,
};

type LinkTail<'a> = (
    Option<(GreenElement, GreenElement, Input<'a>)>,
    GreenElement,
);

fn link_tail(input: Input) -> IResult<Input, LinkTail, ()> {
    alt((
        map(
            tuple((r_bracket_token, l_bracket_token, link_description, r_bracket2_token)),
            |(r_bracket, l_bracket, desc, r_bracket2)| (Some((r_bracket, l_bracket, desc)), r_bracket2),
        ),
        map(r_bracket2_token, |r_bracket2| (None, r_bracket2)),
    ))(input)
}

thread_local! {
    /// While objects are parsed: the start and end address of a stretch of
    /// the text known to hold no `]]`.
    static NO_CLOSE: Cell<Option<Option<(usize, usize)>>> = const { Cell::new(None) };
}

/// Held while one run of objects is parsed, so a description that finds no
/// `]]` is not searched for again from a later `[[` of the same text.
pub struct DescriptionScan(Option<Option<(usize, usize)>>);

impl DescriptionScan {
    pub fn enter() -> DescriptionScan {
        DescriptionScan(NO_CLOSE.with(|memo| memo.replace(Some(None))))
    }
}

impl Drop for DescriptionScan {
    fn drop(&mut self) {
        NO_CLOSE.with(|memo| memo.set(self.0));
    }
}

/// A link description as `org-link-bracket-re` reads it: one character or
/// more, up to the first `]]`.
fn link_description(input: Input) -> IResult<Input, Input, ()> {
    let first = input.s.chars().next().ok_or(nom::Err::Error(()))?;
    let from = input.s.as_ptr() as usize + first.len_utf8();
    let end = input.s.as_ptr() as usize + input.s.len();
    let memo = NO_CLOSE.with(Cell::get);
    if let Some(Some((no_close_from, no_close_end))) = memo {
        if no_close_end == end && no_close_from <= from {
            return Err(nom::Err::Error(()));
        }
    }
    match input.s[first.len_utf8()..].find("]]") {
        Some(at) => Ok(input.take_split(first.len_utf8() + at)),
        None => {
            if memo.is_some() {
                NO_CLOSE.with(|memo| memo.set(Some(Some((from, end)))));
            }
            Err(nom::Err::Error(()))
        }
    }
}

/// The link path, ended where org's `org-link-bracket-re` ends it.
///
/// A run of `n` backslashes before a bracket decides: for `n` 0 or 2 the
/// bracket ends the path, for `n` 1 it is in the path, for `n` >= 3 it is
/// either. There the regex backtracking first tries "in the path" for an odd
/// `n` and "ends the path" for an even `n`, and takes the other reading only
/// when the first one gives no link.
fn link_path(input: Input) -> IResult<Input, Input, ()> {
    let bytes = input.s.as_bytes();
    let closes = |end: usize| end > 0 && link_tail(input.take_split(end).0).is_ok();
    let mut fallbacks = Vec::new();
    let mut end = 0;
    let mut run = 0;
    loop {
        match bytes.get(end) {
            None | Some(b'<' | b'>' | b'\n') => break,
            Some(b'\\') => {
                run += 1;
                end += 1;
                continue;
            }
            Some(b'[' | b']') if run == 0 || run == 2 => break,
            Some(b']') if run >= 3 && run % 2 == 0 => {
                if closes(end) {
                    return Ok(input.take_split(end));
                }
            }
            Some(b']') if run >= 3 => fallbacks.push(end),
            _ => {}
        }
        run = 0;
        end += 1;
    }
    std::iter::once(end)
        .chain(fallbacks.into_iter().rev())
        .find(|&end| closes(end))
        .map(|end| input.take_split(end))
        .ok_or(nom::Err::Error(()))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "debug", skip(input), fields(input = input.s))
)]
pub fn link_node(input: Input) -> IResult<Input, GreenElement, ()> {
    let mut parser = map(
        tuple((l_bracket2_token, link_path, link_tail)),
        |(l_bracket2, path, (desc, r_bracket2))| {
            let mut children = vec![l_bracket2, path.token(LINK_PATH)];

            if let Some((r_bracket, l_bracket, desc)) = desc {
                children.extend([r_bracket, l_bracket]);
                children.extend(link_description_object_nodes(desc));
            }

            children.push(r_bracket2);

            node(LINK, children)
        },
    );
    crate::lossless_parser!(parser, input)
}

#[test]
fn parse() {
    use crate::{ast::Link, tests::to_ast, ParseConfig};

    let to_link = to_ast::<Link>(link_node);

    let link = to_link("[[#id]]");
    insta::assert_debug_snapshot!(
        link.syntax,
        @r###"
    LINK@0..7
      L_BRACKET2@0..2 "[["
      LINK_PATH@2..5 "#id"
      R_BRACKET2@5..7 "]]"
    "###
    );

    let link = to_link("[[#id][desc]]");
    insta::assert_debug_snapshot!(
        link.syntax,
        @r###"
    LINK@0..13
      L_BRACKET2@0..2 "[["
      LINK_PATH@2..5 "#id"
      R_BRACKET@5..6 "]"
      L_BRACKET@6..7 "["
      TEXT@7..11 "desc"
      R_BRACKET2@11..13 "]]"
    "###
    );

    let link = to_link("[[file:/home/dominik/images/jupiter.jpg]]");
    insta::assert_debug_snapshot!(
        link.syntax,
        @r###"
    LINK@0..41
      L_BRACKET2@0..2 "[["
      LINK_PATH@2..39 "file:/home/dominik/im ..."
      R_BRACKET2@39..41 "]]"
    "###
    );

    let link = to_link("[[https://orgmode.org][*bold* description]]");
    insta::assert_debug_snapshot!(
        link.syntax,
        @r###"
    LINK@0..43
      L_BRACKET2@0..2 "[["
      LINK_PATH@2..21 "https://orgmode.org"
      R_BRACKET@21..22 "]"
      L_BRACKET@22..23 "["
      BOLD@23..29
        STAR@23..24 "*"
        TEXT@24..28 "bold"
        STAR@28..29 "*"
      TEXT@29..41 " description"
      R_BRACKET2@41..43 "]]"
    "###
    );

    let config = &ParseConfig::default();

    assert!(link_node(("[[#id][desc]", config).into()).is_err());
}

#[test]
fn a_bracket_in_the_path_is_no_link() {
    use crate::{rowan::ast::AstNode, Org};

    let org = Org::parse("[[a[b]] [[c]]");
    let links: Vec<String> = org
        .document()
        .syntax()
        .descendants()
        .filter(|n| n.kind() == LINK)
        .map(|n| n.to_string())
        .collect();
    assert_eq!(links, vec!["[[c]]".to_string()]);
}

#[cfg(test)]
fn links(text: &str) -> Vec<String> {
    use crate::{rowan::ast::AstNode, Org};

    Org::parse(text)
        .document()
        .syntax()
        .descendants()
        .filter(|n| n.kind() == LINK)
        .map(|n| n.to_string())
        .collect()
}

#[test]
fn a_link_path_ends_where_org_ends_it() {
    // Each row is the link that `emacs -Q` 30.2 (org 9.7.11) reads in a line,
    // `|`, then the line: runs of 0 to 8 backslashes before a bracket in the path.
    let wrong: Vec<String> = include_str!("link_org_9_7_11.txt")
        .lines()
        .filter_map(|row| {
            let (org, line) = row.split_once('|').unwrap();
            let ours = links(line);
            let org: Vec<String> = org.split_terminator('|').map(String::from).collect();
            (ours != org).then(|| format!("{line:?}: org {org:?}, orgize {ours:?}"))
        })
        .collect();
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn a_link_description_ends_where_org_ends_it() {
    // Each row is a line, a tab, then the links `emacs -Q` 30.2 (org 9.7.11)
    // reads in it: each one's text, `\x1e`, its path; joined by `\x1f`.
    let wrong: Vec<String> = include_str!("link_description_org_9_7_11.txt")
        .lines()
        .filter_map(|row| {
            let (line, org) = row.split_once('\t').unwrap();
            let org: Vec<String> = org
                .split_terminator('\x1f')
                .map(|link| link.split_once('\x1e').unwrap().0.to_string())
                .collect();
            let ours = links(line);
            (ours != org).then(|| format!("{line:?}: org {org:?}, orgize {ours:?}"))
        })
        .collect();
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn a_link_path_is_read_in_time_linear_in_its_length() {
    use std::time::{Duration, Instant};

    let units = [
        ("x [[", r"a\\\]"),
        ("x [[", r"a\\\\]"),
        ("x [[", r"a\]"),
        ("", r"[[a\\\]]] "),
        ("", r"[[a\\\\]]] "),
        ("", r"[[a\\\]b][c]] "),
        ("", r"[[a\\\\]b "),
        ("", "[[a][b "),
        ("", "[[a][*b* "),
        ("", "[[a][b]c "),
    ];
    let fastest = |text: &str| -> Duration {
        (0..5)
            .map(|_| {
                let start = Instant::now();
                std::hint::black_box(links(text));
                start.elapsed()
            })
            .min()
            .unwrap()
    };
    let slow: Vec<String> = units
        .iter()
        .filter_map(|(prefix, unit)| {
            let small = fastest(&format!("{prefix}{}", unit.repeat(1_000)));
            let large = fastest(&format!("{prefix}{}", unit.repeat(16_000)));
            let ratio = large.as_secs_f64() / small.as_secs_f64();
            (ratio > 48.0).then(|| format!("{unit:?}: {small:?} -> {large:?} (x{ratio:.1})"))
        })
        .collect();
    assert!(
        slow.is_empty(),
        "16x the text took more than 48x the time:\n{}",
        slow.join("\n")
    );
}
