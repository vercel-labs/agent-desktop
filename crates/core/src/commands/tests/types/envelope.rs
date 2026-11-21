use crate::*;
use serde_json::json;

fn failure(command: &str, error: AdapterError) -> Response {
    Response::err(
        command,
        ErrorPayload::from_app_error(&AppError::Adapter(error)),
    )
}

#[test]
fn success_envelope_shapes() {
    golden!(
        include_str!("../../../../../../tests/fixtures/contract/types-envelope-ok.json"),
        json!({
            "data": Response::ok("snapshot", json!({"app": "Fixture", "ref_count": 3, "complete": true, "tree": {"role": "window"}})),
            "empty": Response::ok("status", json!({})),
        }),
        &["version"]
    );
}

#[test]
fn error_envelope_shapes() {
    golden!(
        include_str!("../../../../../../tests/fixtures/contract/types-envelope-err.json"),
        json!({
            "full": failure("click", AdapterError::new(ErrorCode::StaleRef, "@s8f3k2p9:e4 not found").with_suggestion("Run 'snapshot' to refresh.").with_platform_detail("AXError -25202").with_details(json!({"kind": "ref_not_found"})).with_disposition(DeliverySemantics::not_delivered())),
            "timeout": failure("wait", AdapterError::new(ErrorCode::Timeout, "No answer").with_disposition(DeliverySemantics::not_delivered())),
            "delivered": failure("click", AdapterError::new(ErrorCode::ActionFailed, "Not confirmed").with_disposition(DeliverySemantics::delivered_unverified())),
            "minimal": failure("click", AdapterError::new(ErrorCode::Internal, "Something broke")),
            "non_adapter": Response::err("snapshot", ErrorPayload::from_app_error(&AppError::Internal("unwritable".into()))),
        }),
        &["version"]
    );
}
