use super::*;

const ENVELOPE_DOCS: &[(&str, &str)] = &[
    ("AGENTS.md", include_str!("../../AGENTS.md")),
    (
        "docs/json-output.md",
        include_str!("../../docs/json-output.md"),
    ),
    (
        ".github/ISSUE_TEMPLATE/bug_report.yml",
        include_str!("../../.github/ISSUE_TEMPLATE/bug_report.yml"),
    ),
];

const ABI_DOCS: &[(&str, &str)] = &[
    ("README.md", include_str!("../../README.md")),
    (
        "skills/agent-desktop-ffi/references/build-and-link.md",
        include_str!("../../skills/agent-desktop-ffi/references/build-and-link.md"),
    ),
];

const FFI_HEADER: &str = include_str!("../../crates/ffi/include/agent_desktop.h");
const FFI_API_DOCS: &str = include_str!("../../apps/docs/content/docs/ffi/api.mdx");
const README: &str = include_str!("../../README.md");
const AGENTS_MD: &str = include_str!("../../AGENTS.md");
const WAIT_REFERENCE: &str = include_str!("../../apps/docs/content/docs/commands/wait.mdx");
const HELD_INPUT_COMMANDS: &[&str] = &["key-down", "key-up", "mouse-down", "mouse-up"];

fn quoted_values_after<'a>(text: &'a str, marker: &str) -> Vec<&'a str> {
    text.match_indices(marker)
        .filter_map(|(at, _)| {
            let rest = text[at + marker.len()..].trim_start();
            let rest = rest.strip_prefix('"')?;
            rest.split('"').next()
        })
        .collect()
}

fn integers_after(text: &str, marker: &str) -> Vec<u32> {
    text.match_indices(marker)
        .filter_map(|(at, _)| {
            let digits: String = text[at + marker.len()..]
                .trim_start_matches([' ', '`', '=', '('])
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            digits.parse().ok()
        })
        .collect()
}

#[test]
fn documented_envelope_versions_match_the_code_constant() {
    for (path, text) in ENVELOPE_DOCS {
        let found = [
            quoted_values_after(text, "\"version\":"),
            quoted_values_after(text, "version: "),
        ]
        .concat();
        assert!(
            !found.is_empty(),
            "{path} no longer shows an envelope version"
        );
        for version in found {
            assert_eq!(
                version,
                agent_desktop_core::response::output::ENVELOPE_VERSION,
                "{path} documents envelope version {version}"
            );
        }
    }
}

#[test]
fn documented_abi_major_matches_the_header() {
    let header_major = integers_after(FFI_HEADER, "#define AD_ABI_VERSION_MAJOR")[0];
    for (path, text) in ABI_DOCS {
        let found = [
            integers_after(text, "ad_init("),
            integers_after(text, "AD_ABI_VERSION_MAJOR"),
            integers_after(text, "AD_ABI_VERSION_MAJOR` is currently"),
            integers_after(text, "the ABI major, currently"),
            integers_after(text, "ABI major is"),
        ]
        .concat();
        assert!(!found.is_empty(), "{path} no longer states the ABI major");
        for major in found {
            assert_eq!(major, header_major, "{path} documents ABI major {major}");
        }
    }
}

fn header_enum_constants(header: &str) -> Vec<(&str, i64)> {
    header
        .lines()
        .filter_map(|line| {
            let (name, value) = line.trim().split_once(" = ")?;
            if !name.starts_with("AD_") {
                return None;
            }
            let value = value.strip_suffix(',')?.parse().ok()?;
            Some((name, value))
        })
        .collect()
}

#[test]
fn documented_ffi_enum_constants_match_the_header() {
    let constants = header_enum_constants(FFI_HEADER);
    assert!(
        constants.len() >= 100,
        "agent_desktop.h parsing found {} enum constants, expected the full set",
        constants.len()
    );
    for (name, value) in constants {
        let documented = format!("<code>{name}</code> {value}");
        assert!(
            FFI_API_DOCS.contains(&documented),
            "apps/docs/content/docs/ffi/api.mdx must list {documented}"
        );
    }
}

#[test]
fn documented_command_counts_match_the_cli() {
    let total = cli_command_names()
        .iter()
        .filter(|name| name.as_str() != "help")
        .count();
    let operational = total - HELD_INPUT_COMMANDS.len();
    assert!(
        README.contains(&format!(
            "{total} command names, {operational} operational commands"
        )),
        "README.md must state {total} command names and {operational} operational commands"
    );
    assert!(
        AGENTS_MD.contains(&format!("{total} commands spanning")),
        "AGENTS.md must state {total} commands"
    );
    assert!(
        AGENTS_MD.contains(&format!(
            "{operational} of the {total} command names are operational"
        )),
        "AGENTS.md must state {operational} of the {total} command names are operational"
    );
}

/// The wait reference documents every flag the `wait` subcommand accepts, so a
/// new wait mode cannot ship without its line in the reference. The flags come
/// from the clap tree the binary runs, never from a list kept beside the docs.
#[test]
fn wait_reference_documents_every_wait_flag() {
    let wait = Cli::command()
        .get_subcommands()
        .find(|command| command.get_name() == "wait")
        .expect("the wait subcommand exists")
        .clone();
    let mut flags = wait
        .get_arguments()
        .filter(|argument| !matches!(argument.get_id().as_str(), "help" | "version"))
        .filter_map(|argument| argument.get_long())
        .collect::<Vec<_>>();
    flags.sort_unstable();
    assert!(
        flags.contains(&"surface"),
        "wait accepts --surface: {flags:?}"
    );
    for flag in flags {
        assert!(
            documents_flag(WAIT_REFERENCE, flag),
            "apps/docs/content/docs/commands/wait.mdx does not document --{flag}"
        );
    }
}

/// Whether `reference` names `flag` as a whole flag: a `--window` line does not
/// document `--window-id`, and a longer flag containing the name documents
/// nothing.
fn documents_flag(reference: &str, flag: &str) -> bool {
    let named = format!("--{flag}");
    reference.match_indices(&named).any(|(at, matched)| {
        reference[at + matched.len()..]
            .chars()
            .next()
            .is_none_or(|next| !(next.is_ascii_alphanumeric() || next == '-'))
    })
}
