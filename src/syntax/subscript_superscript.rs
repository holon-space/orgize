use std::{cell::RefCell, collections::HashMap};

use memchr::memchr2_iter;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_while1},
    combinator::opt,
    IResult, InputTake,
};

use crate::{
    syntax::{
        combinator::{caret_token, underscore_token},
        object::standard_object_nodes,
    },
    SyntaxKind,
};

use super::{
    combinator::{l_curly_token, node, r_curly_token, GreenElement},
    emphasis::is_org_space,
    input::Input,
};

pub fn superscript_node(input: Input) -> IResult<Input, GreenElement, ()> {
    let (input, caret) = caret_token(input)?;

    let mut children = vec![caret];

    if input.c.use_sub_superscript.is_brace() {
        let (input, rest) = template1(input)?;
        children.extend(rest);
        return Ok((input, node(SyntaxKind::SUPERSCRIPT, children)));
    }

    let (input, rest) = alt((template0, template1, template2))(input)?;
    children.extend(rest);

    Ok((input, node(SyntaxKind::SUPERSCRIPT, children)))
}

pub fn subscript_node(input: Input) -> IResult<Input, GreenElement, ()> {
    let (input, underscore) = underscore_token(input)?;

    let mut children = vec![underscore];

    if input.c.use_sub_superscript.is_brace() {
        let (input, rest) = template1(input)?;
        children.extend(rest);
        return Ok((input, node(SyntaxKind::SUBSCRIPT, children)));
    }

    let (input, rest) = alt((template0, template1, template2))(input)?;
    children.extend(rest);

    Ok((input, node(SyntaxKind::SUBSCRIPT, children)))
}

fn template0(input: Input) -> IResult<Input, Vec<GreenElement>, ()> {
    let (input, star) = tag("*")(input)?;
    Ok((input, vec![star.text_token()]))
}

fn template1(input: Input) -> IResult<Input, Vec<GreenElement>, ()> {
    let (input, l) = l_curly_token(input)?;
    let (input, contents) = balanced_brackets(input)?;
    let (input, r) = r_curly_token(input)?;
    let mut children = vec![];
    children.push(l);
    children.extend(standard_object_nodes(contents));
    children.push(r);
    Ok((input, children))
}

fn template2(input: Input) -> IResult<Input, Vec<GreenElement>, ()> {
    let (input, sign) = opt(alt((tag("+"), tag("-"))))(input)?;

    let (input, contents) =
        take_while1(|c: char| c.is_alphanumeric() || c == ',' || c == '\\' || c == '.')(input)?;

    if contents.s.ends_with(|c: char| !c.is_alphanumeric()) {
        return Err(nom::Err::Error(()));
    }

    let mut children = vec![];

    if let Some(s) = sign {
        children.push(s.text_token())
    }

    children.push(contents.text_token());

    Ok((input, children))
}

thread_local! {
    static BRACES: RefCell<Option<Option<Braces>>> = const { RefCell::new(None) };
}

/// Held while one run of objects is parsed, so the braces of its text are
/// paired once, not once per `{`.
pub struct BraceScan(Option<Option<Braces>>);

impl BraceScan {
    pub fn enter() -> BraceScan {
        BraceScan(BRACES.with(|memo| memo.replace(Some(None))))
    }
}

impl Drop for BraceScan {
    fn drop(&mut self) {
        BRACES.with(|memo| *memo.borrow_mut() = self.0.take());
    }
}

/// The braces of a text from the `{` at `from` to `end`: the address of each
/// `{` and of the `}` that closes it.
struct Braces {
    from: usize,
    end: usize,
    closes: HashMap<usize, usize>,
}

impl Braces {
    /// `input` starts right after the `{` at `from`.
    fn pair(from: usize, input: Input) -> Braces {
        let bytes = input.as_bytes();
        let start = from + 1;
        let mut open = vec![from];
        let mut closes = HashMap::new();
        for i in memchr2_iter(b'{', b'}', bytes) {
            if bytes[i] == b'{' {
                open.push(start + i);
            } else if let Some(o) = open.pop() {
                closes.insert(o, start + i);
            }
        }
        Braces {
            from,
            end: start + bytes.len(),
            closes,
        }
    }
}

/// The contents up to the `}` that closes the `{` right before `input`.
fn balanced_brackets(input: Input) -> IResult<Input, Input, ()> {
    let open = input.s.as_ptr() as usize - 1;
    let close = BRACES.with(|memo| {
        let mut memo = memo.borrow_mut();
        let close = |braces: &Braces| braces.closes.get(&open).copied();
        match &mut *memo {
            Some(Some(braces)) if braces.from <= open => {
                assert_eq!(
                    braces.end,
                    open + 1 + input.s.len(),
                    "a brace is paired inside the run of objects it is parsed in"
                );
                close(braces)
            }
            Some(braces) => close(braces.insert(Braces::pair(open, input))),
            None => close(&Braces::pair(open, input)),
        }
    });
    match close {
        Some(close) => Ok(input.take_split(close - open - 1)),
        None => Err(nom::Err::Error(())),
    }
}

/// org's `org-match-substring-regexp`: a character that is not
/// `[[:space:]]` before the `_` or `^`.
pub fn verify_pre(i: &Input) -> bool {
    !i.c.use_sub_superscript.is_nil() && i.s.chars().next_back().is_some_and(|c| !is_org_space(c))
}

#[test]
fn parse() {
    use crate::ast::Subscript;
    use crate::config::{ParseConfig, UseSubSuperscript};
    use crate::tests::to_ast;

    let to_subscript = to_ast::<Subscript>(subscript_node);

    insta::assert_debug_snapshot!(
        to_subscript("_*").syntax,
        @r###"
    SUBSCRIPT@0..2
      UNDERSCORE@0..1 "_"
      TEXT@1..2 "*"
    "###
    );

    insta::assert_debug_snapshot!(
        to_subscript("_{*bo\nld*}").syntax,
        @r###"
    SUBSCRIPT@0..10
      UNDERSCORE@0..1 "_"
      L_CURLY@1..2 "{"
      BOLD@2..9
        STAR@2..3 "*"
        TEXT@3..8 "bo\nld"
        STAR@8..9 "*"
      R_CURLY@9..10 "}"
    "###
    );

    insta::assert_debug_snapshot!(
        to_subscript("_+123").syntax,
        @r###"
    SUBSCRIPT@0..5
      UNDERSCORE@0..1 "_"
      TEXT@1..2 "+"
      TEXT@2..5 "123"
    "###
    );

    insta::assert_debug_snapshot!(
        to_subscript("_abc").syntax,
        @r###"
    SUBSCRIPT@0..4
      UNDERSCORE@0..1 "_"
      TEXT@1..4 "abc"
    "###
    );

    let with_brace = ParseConfig {
        use_sub_superscript: UseSubSuperscript::Brace,
        ..Default::default()
    };

    debug_assert!(subscript_node(("_*", &with_brace).into()).is_err());
    debug_assert!(subscript_node(("_abc", &with_brace).into()).is_err());
    debug_assert!(subscript_node(("_+123", &with_brace).into()).is_err());
    debug_assert!(subscript_node(("_{*bo\nld*}", &with_brace).into()).is_ok());
}
