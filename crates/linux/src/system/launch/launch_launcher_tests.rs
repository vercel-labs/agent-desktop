use super::launcher_words;

fn words(items: &[&str]) -> Vec<String> {
    items.iter().map(|word| (*word).to_owned()).collect()
}

#[test]
fn a_single_letter_switch_of_the_entry_is_not_part_of_the_identity() {
    assert_eq!(
        launcher_words(&words(&["/usr/bin/kate", "-b"])),
        words(&["/usr/bin/kate"])
    );
}

#[test]
fn long_options_and_positional_words_still_name_the_application() {
    let entry = words(&["/usr/bin/python3", "/opt/app.py", "--gui", "--mode=x", "-"]);

    assert_eq!(launcher_words(&entry), entry);
}

#[test]
fn the_program_word_is_never_dropped() {
    assert_eq!(launcher_words(&words(&["-x", "-b"])), words(&["-x"]));
}
