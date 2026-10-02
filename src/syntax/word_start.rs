/// Characters that join the next Latin letter into one word in an org-mode
/// buffer, as inclusive ranges in ascending order.
const WORD_CHARS: &[(char, char)] = &include!("word_chars_org_9_7_11.rs");

/// Whether org's `\<` matches between `before` and a Latin letter, as the
/// inline source block and inline call parsers require.
pub fn follows_no_word(before: &str) -> bool {
    before.chars().next_back().map_or(true, |c| {
        WORD_CHARS
            .binary_search_by(|&(low, high)| {
                if high < c {
                    std::cmp::Ordering::Less
                } else if low > c {
                    std::cmp::Ordering::Greater
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .is_err()
    })
}

#[test]
fn the_word_chars_are_ascending_and_disjoint() {
    assert!(WORD_CHARS.iter().all(|(low, high)| low <= high));
    assert!(WORD_CHARS.windows(2).all(|w| w[0].1 < w[1].0));
}
