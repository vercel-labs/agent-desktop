use super::{MAX_CURSOR_LABEL_BYTES, MAX_CURSOR_LABEL_WORDS, clamp_cursor_label};

#[test]
fn a_caption_of_only_whitespace_says_nothing() {
    assert_eq!(clamp_cursor_label("   \t \n "), None);
}

#[test]
fn a_caption_past_the_byte_ceiling_is_cut_on_a_character_boundary() {
    let clamped = clamp_cursor_label(&"日".repeat(1_000)).expect("still says something");
    assert!(clamped.len() <= MAX_CURSOR_LABEL_BYTES);
    assert_eq!(clamped.chars().count() * "日".len(), clamped.len());
}

#[test]
fn a_caption_past_the_word_ceiling_is_cut_and_marked() {
    let many = (0..MAX_CURSOR_LABEL_WORDS + 5)
        .map(|index| format!("word{index}"))
        .collect::<Vec<_>>()
        .join(" ");
    let clamped = clamp_cursor_label(&many).expect("still says something");
    assert!(clamped.ends_with('…'));
    assert_eq!(clamped.split_whitespace().count(), MAX_CURSOR_LABEL_WORDS);
}
