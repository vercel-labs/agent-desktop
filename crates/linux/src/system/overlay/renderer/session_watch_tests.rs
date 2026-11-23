use super::*;

fn body(text: &str) -> io::Result<Vec<u8>> {
    Ok(text.as_bytes().to_vec())
}

#[test]
fn a_manifest_reads_as_live_only_while_its_overlay_is_on() {
    assert_eq!(
        classify(body(r#"{"cursor_overlay": {"enabled": true}}"#)),
        SessionReading::Live
    );
    assert_eq!(
        classify(body(r#"{"cursor_overlay": {"enabled": false}}"#)),
        SessionReading::Finished
    );
    assert_eq!(
        classify(body(r#"{"id": "s1"}"#)),
        SessionReading::Finished,
        "a disabled overlay is not serialized"
    );
    assert_eq!(
        classify(body(
            r#"{"ended_at": 5, "cursor_overlay": {"enabled": true}}"#
        )),
        SessionReading::Finished
    );
}

#[test]
fn a_missing_manifest_is_finished_and_an_unreadable_one_is_unknown() {
    assert_eq!(
        classify(Err(io::Error::from(io::ErrorKind::NotFound))),
        SessionReading::Finished
    );
    assert_eq!(
        classify(Err(io::Error::from(io::ErrorKind::PermissionDenied))),
        SessionReading::Unknown
    );
    assert_eq!(classify(body("{\"cursor_ov")), SessionReading::Unknown);
}

#[test]
fn a_session_ends_after_two_finished_readings_in_a_row() {
    let mut watch = SessionWatch::default();
    assert!(!watch.observe("s1", SessionReading::Finished));
    assert!(
        !watch.observe("s1", SessionReading::Unknown),
        "an unknown reading starts the count again"
    );
    assert!(!watch.observe("s1", SessionReading::Finished));
    assert!(
        !watch.observe("s2", SessionReading::Finished),
        "sessions are counted apart"
    );
    assert!(watch.observe("s1", SessionReading::Finished));
}
