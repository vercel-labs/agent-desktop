use super::*;

#[test]
fn the_refusal_names_the_running_pids_and_sends_nothing() {
    let error = already_running("Notes", &[ProcessId::new(41), ProcessId::new(42)]);

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    let details = error.details.expect("details name the running pids");
    assert_eq!(details["pid"], 41);
    assert_eq!(details["candidate_pids"], serde_json::json!([41, 42]));
    assert!(
        error
            .suggestion
            .as_deref()
            .is_some_and(|text| text.contains("without --no-attach") && text.contains("close"))
    );
}
