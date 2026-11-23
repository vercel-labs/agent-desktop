use super::*;
use agent_desktop_core::ErrorCode;

#[test]
fn an_undrawable_overlay_keeps_rendered_false_and_names_its_cause() {
    for unavailable in [Unavailable::Absent, Unavailable::Incompatible] {
        let error = unrendered(unavailable);
        assert_eq!(error.code, ErrorCode::PlatformNotSupported);
        let details = error.details.expect("details");
        assert_eq!(details["rendered"], false);
        assert_eq!(details["cause"], unavailable.cause());
        assert_eq!(
            error.suggestion.as_deref(),
            Some(unavailable.suggestion().as_str())
        );
    }
}
