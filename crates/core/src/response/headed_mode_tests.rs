use std::path::Path;

use super::headed_mode_suggestion;

#[test]
fn headed_guidance_requires_the_users_request_and_names_the_effect() {
    for effect in [
        "takes the foreground",
        "moves the pointer",
        "sends synthetic keys",
        "shows a prompt over the user's window",
    ] {
        let suggestion = headed_mode_suggestion(effect);
        assert!(suggestion.contains(effect), "{suggestion}");
        assert!(
            suggestion.contains("Use --headed only if the user explicitly asked for that"),
            "{suggestion}"
        );
        assert!(
            suggestion.contains("otherwise report this limitation to the user"),
            "{suggestion}"
        );
    }
}

#[test]
fn refusal_suggestions_never_recommend_headed_without_a_user_request() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates");
    scan_refusals(&root);
}

fn scan_refusals(path: &Path) {
    for entry in std::fs::read_dir(path).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap();
        if name == "tests" || name.ends_with("_tests.rs") {
            continue;
        }
        if path.is_dir() {
            scan_refusals(&path);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            let source = std::fs::read_to_string(&path).unwrap();
            let production = source.split("#[cfg(test)]").next().unwrap();
            for (index, line) in production.lines().enumerate() {
                let line = line.trim();
                if !line.contains("--headed") || line.starts_with("//") {
                    continue;
                }
                let rejects_headed = line.contains("without --app, --window-id or --headed")
                    || line.contains("instead of the --headed route");
                assert!(
                    rejects_headed || line.contains("only if the user explicitly asked"),
                    "{}:{} recommends headed mode without a user request: {line}",
                    path.display(),
                    index + 1
                );
            }
        }
    }
}
