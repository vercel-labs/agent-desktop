use agent_desktop_core::ErrorCode;

use super::{CenterShape, gate_landmarks, unrecognized_center_error};

#[test]
fn a_center_carrying_the_notification_list_walks_it() {
    assert!(matches!(
        gate_landmarks(true, false),
        Ok(CenterShape::WalkList)
    ));
    assert!(
        matches!(gate_landmarks(true, true), Ok(CenterShape::WalkList)),
        "the list wins whenever it is present, whatever the empty-state landmarks do"
    );
}

#[test]
fn an_empty_center_is_a_legitimate_zero_entry_answer() {
    assert!(matches!(
        gate_landmarks(false, true),
        Ok(CenterShape::EmptyCenter)
    ));
}

#[test]
fn a_center_with_no_landmarks_is_refused_not_listed_as_empty() {
    let shape = gate_landmarks(false, false).expect_err("an unrecognized tree must be refused");

    assert_eq!(shape.code, ErrorCode::PlatformNotSupported);
    assert_eq!(
        unrecognized_center_error().code,
        ErrorCode::PlatformNotSupported,
        "the refusal is the landmark gate's own answer, not a silent empty listing"
    );
}

#[test]
fn the_refusal_names_the_build_and_the_missing_landmark() {
    let error = unrecognized_center_error();

    let detail = error
        .platform_detail
        .as_deref()
        .expect("the refusal must carry its reason");
    assert!(
        detail.contains("MainListView"),
        "the detail names the missing landmark's AutomationId: {detail}"
    );
    assert!(
        detail.contains("Windows build"),
        "the detail names the build the shape was measured against: {detail}"
    );
}

#[cfg(target_os = "windows")]
mod classification {
    use super::super::{classify_find, classify_find_all, gate_landmarks};
    use crate::system::runtime::hresult::{UIA_E_ELEMENTNOTAVAILABLE, UIA_E_TIMEOUT};
    use crate::tree::automation::{ERR_NOTFOUND, ERR_TIMEOUT};
    use agent_desktop_core::ErrorCode;
    use uiautomation::Error;

    fn hresult_error(code: i32) -> Error {
        Error::from(windows::core::HRESULT(code))
    }

    /// Exhaustion is what a real `find_first` on a region with no match
    /// reports, so the not-found answer must stay `None` - the same answer
    /// the old blanket mapping gave, minus the faults it also swallowed.
    #[test]
    fn a_genuinely_not_found_search_is_absence_not_a_fault() {
        let exhausted = Error::from(windows::core::Error::empty());
        assert!(
            classify_find(Err(exhausted), "probe")
                .expect("exhaustion is absence")
                .is_none()
        );
        assert!(
            classify_find(Err(Error::new(ERR_NOTFOUND, "test")), "probe")
                .expect("a raced-away subtree is absence")
                .is_none()
        );
        assert!(
            classify_find(Err(hresult_error(UIA_E_ELEMENTNOTAVAILABLE)), "probe")
                .expect("a vanished element is absence")
                .is_none()
        );
        assert!(
            classify_find_all(Err(Error::new(ERR_NOTFOUND, "test")), "probe")
                .expect("absence for the multi-match search too")
                .is_empty()
        );
    }

    /// The transient faults the old `Err(_) => Ok(None)` mapping swallowed:
    /// each must surface with its own code, never read as a confident
    /// "the center does not carry it".
    #[test]
    fn a_transient_search_failure_is_an_honest_error_not_a_confident_negative() {
        for error in [
            Error::new(ERR_TIMEOUT, "test"),
            hresult_error(UIA_E_TIMEOUT),
        ] {
            let surfaced = match classify_find(Err(error), "probe") {
                Err(surfaced) => surfaced,
                Ok(_) => panic!("a transient fault must surface, never read as absence"),
            };
            assert_eq!(surfaced.code, ErrorCode::Timeout);
        }
        let emptied = match classify_find_all(Err(Error::new(ERR_TIMEOUT, "test")), "probe") {
            Err(surfaced) => surfaced,
            Ok(_) => panic!("a transient fault must surface, never read as an empty match set"),
        };
        assert_eq!(emptied.code, ErrorCode::Timeout);
    }

    /// The landmark gate's distinction: absence (what a missing landmark
    /// reports) flows into the gate and is refused as an unrecognized tree -
    /// PLATFORM_NOT_SUPPORTED, never an empty listing - while a transient
    /// fault never reaches the gate at all because the search surfaces
    /// first. Reverting the search classification to blanket-absence fails
    /// `a_transient_search_failure_is_an_honest_error_not_a_confident_negative`.
    #[test]
    fn an_absent_landmark_reaches_the_gate_which_refuses_rather_than_lists_empty() {
        let absent = classify_find(Err(Error::new(ERR_NOTFOUND, "test")), "probe")
            .expect("a missing landmark is absence");
        assert!(absent.is_none());

        let shape = gate_landmarks(absent.is_some(), false)
            .expect_err("a center with no landmarks must be refused");
        assert_eq!(shape.code, ErrorCode::PlatformNotSupported);
    }
}
