use super::*;

#[test]
fn shipped_ref_passes_live_preflight_when_only_the_token_format_changed() {
    let (mut saved, mut adapter) = target("textfield", &[capability::SET_VALUE]);
    saved.process.process_instance = Some("macos-proc-v1:10:20".into());
    saved.identity.retained_object = Some("axtok1:abcd".into());
    adapter.0.identity.identifiers =
        adapter
            .0
            .identity
            .identifiers
            .clone()
            .with_retained_object(format!(
                "axtok2:{}:macos-proc-v1:10:20:abcd",
                saved.process.pid
            ));
    assert!(
        evaluate(
            &saved,
            &adapter,
            ActionRequest::headless(Action::SetValue("new text".into()))
        )
        .is_ok()
    );

    adapter.0.identity.identifiers = crate::IdentifierEvidence::absent().with_retained_object(
        format!("axtok2:{}:macos-proc-v1:11:20:abcd", saved.process.pid),
    );
    let error = evaluate(
        &saved,
        &adapter,
        ActionRequest::headless(Action::SetValue("new text".into())),
    )
    .unwrap_err();
    assert_eq!(error.code, crate::ErrorCode::StaleRef);
    assert_eq!(error.disposition, crate::DeliverySemantics::not_delivered());
}
