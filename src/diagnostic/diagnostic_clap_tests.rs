use super::{clap_error_suggestion, clap_error_summary};

/// The shape clap emits for a missing positional: the headline carries no
/// argument name, so a summary built from it alone cannot say what is missing.
const MISSING_POSITIONAL: &str = "error: the following required arguments were not provided:\n  <VALUE>\n\nUsage: agent-desktop.exe set-value <REF> <VALUE>\n\nFor more information, try '--help'.";

#[test]
fn a_missing_argument_summary_names_the_argument() {
    let summary = clap_error_summary(MISSING_POSITIONAL);

    assert!(
        summary.contains("<VALUE>"),
        "the caller cannot act on a message that omits the missing argument, got: {summary}"
    );
    assert!(
        !summary.contains("Usage:"),
        "usage boilerplate is not actionable for a machine consumer, got: {summary}"
    );
    assert!(
        !summary.contains('\n'),
        "the envelope carries a single-line message, got: {summary}"
    );
}

#[test]
fn a_single_line_error_is_unchanged_and_an_empty_one_still_says_something() {
    assert_eq!(
        clap_error_summary("error: unexpected argument '--nope' found"),
        "error: unexpected argument '--nope' found"
    );
    assert_eq!(clap_error_summary(""), "parse error");
    assert_eq!(clap_error_summary("\n\nUsage: x"), "parse error");
}

const UNEXPECTED_POSITIONAL: &str = "error: unexpected argument 'selected' found\n\nUsage: agent-desktop click [OPTIONS] <REF>\n\nFor more information, try '--help'.";

const TIPPED_PARSE_ERROR: &str = "error: unexpected argument '--nope' found\n\n  tip: to pass '--nope' as a value, use '-- --nope'\n\nUsage: agent-desktop click <REF>\n\nFor more information, try '--help'.";

#[test]
fn a_parse_error_suggestion_carries_usage_and_the_subcommand_help_pointer() {
    let suggestion = clap_error_suggestion(UNEXPECTED_POSITIONAL, "click")
        .expect("a usage line is enough for a suggestion");

    assert!(
        suggestion.contains("Usage: agent-desktop click [OPTIONS] <REF>"),
        "the suggestion must carry clap's usage line, got: {suggestion}"
    );
    assert!(
        suggestion.contains("Run `agent-desktop click --help` for every argument."),
        "the suggestion must point at the subcommand help, got: {suggestion}"
    );
}

#[test]
fn a_parse_error_suggestion_leads_with_claps_tip() {
    let suggestion = clap_error_suggestion(TIPPED_PARSE_ERROR, "click")
        .expect("a tip and a usage line yield a suggestion");

    assert!(
        suggestion.starts_with("tip: to pass '--nope'"),
        "clap's tip is the most actionable line and must come first, got: {suggestion}"
    );
    assert!(
        suggestion.contains("Usage: agent-desktop click <REF>"),
        "the usage line follows the tip, got: {suggestion}"
    );
}

const TOP_LEVEL_PARSE_ERROR: &str = "error: unrecognized subcommand 'clik'\n\n  tip: a similar subcommand exists: 'click'\n\nUsage: agent-desktop [OPTIONS] [COMMAND]\n\nFor more information, try '--help'.";

#[test]
fn a_parse_error_without_a_subcommand_omits_the_help_pointer() {
    let suggestion = clap_error_suggestion(TOP_LEVEL_PARSE_ERROR, "unknown")
        .expect("a tip and a usage line yield a suggestion");

    assert!(suggestion.starts_with("tip: a similar subcommand exists"));
    assert!(
        !suggestion.contains("--help"),
        "no identified subcommand means no subcommand help to point at, got: {suggestion}"
    );
}

#[test]
fn a_nested_command_points_at_its_own_help() {
    let message = "error: unexpected argument '--bogus' found\n\nUsage: agent-desktop.exe session start [OPTIONS]\n\nFor more information, try '--help'.";
    let suggestion = clap_error_suggestion(message, "session").expect("a usage line");

    assert!(
        suggestion.ends_with("Run `agent-desktop session start --help` for every argument."),
        "the pointer must name the nested command, got: {suggestion}"
    );
}

#[test]
fn a_parse_error_without_tip_or_usage_still_points_at_the_command_help() {
    let message = "error: a value is required for '--app <APP>' but none was supplied\n\nFor more information, try '--help'.";

    assert_eq!(
        clap_error_suggestion(message, "snapshot").as_deref(),
        Some("Run `agent-desktop snapshot --help` for every argument.")
    );
    assert!(clap_error_suggestion(message, "unknown").is_none());
}
