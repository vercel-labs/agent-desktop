pub fn headed_mode_suggestion(effect: &str) -> String {
    format!(
        "This operation needs headed mode, which {effect}. Use --headed only if the user explicitly asked for that; otherwise report this limitation to the user."
    )
}

#[cfg(test)]
#[path = "headed_mode_tests.rs"]
mod tests;
