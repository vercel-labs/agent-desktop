pub fn headed_mode_suggestion(effect: &str) -> String {
    format!(
        "This operation needs headed mode, which {effect}. Use --headed only if the user explicitly asked for that; otherwise report this limitation to the user."
    )
}

pub(crate) fn window_focus_suggestion(policy: crate::InteractionPolicy) -> String {
    if policy.is_headed() {
        "Make sure the named window stays active before you retry; a sibling window or another application's menu can hold the keyboard".to_owned()
    } else {
        format!(
            "Bring the named window to the front yourself, or tell the user it is not in front. {}",
            headed_mode_suggestion("brings the named window forward before sending keys")
        )
    }
}

#[cfg(test)]
#[path = "headed_mode_tests.rs"]
mod tests;
