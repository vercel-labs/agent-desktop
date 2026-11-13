use agent_desktop_core::{AdapterError, Deadline, ErrorCode};

#[test]
fn hard_ax_error_is_not_a_success_and_does_not_abort_the_strategy_list() {
    let deadline = Deadline::standard().expect("deadline");

    let failed = super::strategy_succeeded(
        Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "AXDismiss failed",
        )),
        deadline,
    );
    let succeeded = super::strategy_succeeded(Ok(true), deadline);

    assert!(!failed.expect("a hard AX error falls through to the next strategy"));
    assert!(succeeded.expect("strategy result"));
}

#[test]
fn permission_denial_is_terminal_for_dismiss_strategies() {
    let result = super::strategy_succeeded(
        Err(AdapterError::permission_denied()),
        Deadline::standard().expect("deadline"),
    );

    assert_eq!(
        result.expect_err("permission denial must abort").code,
        ErrorCode::PermDenied
    );
}

#[test]
fn expired_deadline_is_terminal_for_dismiss_strategies() {
    let deadline = Deadline::after(0).expect("deadline");
    let result = super::strategy_succeeded(
        Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "AXDismiss failed",
        )),
        deadline,
    );

    assert_eq!(
        result.expect_err("expired deadline must abort").code,
        ErrorCode::Timeout
    );
}

#[test]
fn a_failure_after_a_dispatched_step_reports_unverified() {
    let error = super::tracked_attempt(|delivery| {
        delivery.mark_delivered();
        Err(AdapterError::policy_denied_for_policy(
            "the pointer may not move",
            agent_desktop_core::InteractionPolicy::focus_fallback(),
        ))
    })
    .expect_err("the entry attempt was refused");

    assert_eq!(
        error.disposition,
        agent_desktop_core::DeliverySemantics::delivered_unverified()
    );
}

#[test]
fn a_failure_before_any_dispatched_step_stays_not_delivered() {
    let error = super::tracked_attempt(|_| {
        Err(AdapterError::policy_denied_for_policy(
            "the pointer may not move",
            agent_desktop_core::InteractionPolicy::focus_fallback(),
        ))
    })
    .expect_err("the entry attempt was refused");

    assert_eq!(
        error.disposition,
        agent_desktop_core::DeliverySemantics::not_delivered()
    );
}

#[test]
fn a_dispatched_action_with_an_uncertain_outcome_marks_delivery_before_the_timeout() {
    let outcome: Result<bool, AdapterError> = Err(AdapterError::new(
        ErrorCode::AppUnresponsive,
        "AXDismiss returned kAXErrorCannotComplete; mutation outcome is uncertain",
    )
    .with_disposition(agent_desktop_core::DeliverySemantics::uncertain()));

    let error = super::tracked_attempt(|delivery| {
        delivery.mark_ax_attempt(&outcome);
        Err(Deadline::after(0).expect("deadline").timeout_error())
    })
    .expect_err("the entry attempt failed");

    assert_eq!(
        error.disposition,
        agent_desktop_core::DeliverySemantics::delivered_unverified()
    );
    assert_eq!(
        error.disposition.retry(),
        agent_desktop_core::RetryDisposition::Unsafe
    );
}

#[test]
fn an_ax_action_refused_before_dispatch_leaves_the_entry_not_delivered() {
    let element = crate::tree::AXElement(std::ptr::null_mut());
    let deadline = Deadline::standard().expect("deadline");

    let error = super::tracked_attempt(|delivery| {
        let outcome = super::tracked_ax_action(&element, "AXDismiss", delivery, deadline);
        assert!(
            matches!(
                &outcome,
                Err(error)
                    if error.disposition
                        == agent_desktop_core::DeliverySemantics::not_delivered()
            ),
            "a null element is refused before the perform call: {outcome:?}"
        );
        Err(Deadline::after(0).expect("deadline").timeout_error())
    })
    .expect_err("the entry attempt failed");

    assert_eq!(
        error.disposition,
        agent_desktop_core::DeliverySemantics::not_delivered()
    );
    assert_eq!(
        error.disposition.retry(),
        agent_desktop_core::RetryDisposition::Safe
    );
}

#[test]
fn an_action_the_element_does_not_support_stays_not_delivered() {
    let outcome: Result<bool, AdapterError> = Ok(false);

    let error = super::tracked_attempt(|delivery| {
        delivery.mark_ax_attempt(&outcome);
        Err(Deadline::after(0).expect("deadline").timeout_error())
    })
    .expect_err("the entry attempt failed");

    assert_eq!(
        error.disposition,
        agent_desktop_core::DeliverySemantics::not_delivered()
    );
    assert_eq!(
        error.disposition.retry(),
        agent_desktop_core::RetryDisposition::Safe
    );
}
