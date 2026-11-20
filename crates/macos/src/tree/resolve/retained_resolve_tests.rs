use super::{
    RetainedFacts, RetainedMismatch, native_identifier_attribute, resolve_saved_token,
    token_mismatch,
};
use agent_desktop_core::{
    Deadline, ElementIdentifier, IdentifierKind, ProcessId, RefCapabilities, RefEntry,
    RefEntryIdentity, RefGeometry, RefProcess, RefScope, RefSource, SnapshotSurface,
};

fn entry(role: &str) -> RefEntry {
    RefEntry {
        process: RefProcess {
            pid: ProcessId::new(7),
            process_instance: Some("test-instance".into()),
        },
        identity: RefEntryIdentity {
            unique_at_capture: false,
            retained_object: None,
            role: role.into(),
            name: Some("Investors".into()),
            value: None,
            description: None,
            native_id: None,
        },
        geometry: RefGeometry {
            bounds_space: None,
            bounds: None,
            bounds_hash: None,
        },
        capabilities: RefCapabilities {
            states: vec![],
            available_actions: vec![],
        },
        source: RefSource {
            source_app: None,
            source_window_id: Some("w-10".into()),
            source_window_title: None,
            source_window_bounds_hash: None,
            source_surface: SnapshotSurface::Window,
            renderer_accessibility_forced: false,
        },
        scope: RefScope {
            root_ref: None,
            path_is_absolute: false,
            path: smallvec::smallvec![0, 1],
        },
    }
}

fn facts<'a>(
    pid: i32,
    role: Option<&'a str>,
    subrole: Option<&'a str>,
    native_id: Option<&'a str>,
) -> RetainedFacts<'a> {
    RetainedFacts {
        pid,
        role,
        subrole,
        native_id,
    }
}

#[test]
fn token_mismatch_accepts_only_the_saved_element() {
    let stored = entry("button");

    assert_eq!(
        token_mismatch(&stored, &facts(7, Some("AXButton"), None, None)),
        None
    );
    assert_eq!(
        token_mismatch(
            &entry("treeitem"),
            &facts(7, Some("AXRow"), Some("AXOutlineRow"), None)
        ),
        None
    );
    assert_eq!(
        token_mismatch(&stored, &facts(8, Some("AXButton"), None, None)),
        Some(RetainedMismatch::Pid)
    );
    for role in [Some("AXCell"), None, Some("AXRoleThatDoesNotExist")] {
        assert_eq!(
            token_mismatch(&stored, &facts(7, role, None, None)),
            Some(RetainedMismatch::Role)
        );
    }

    let mut identified = entry("button");
    identified.identity.native_id = Some(ElementIdentifier {
        kind: IdentifierKind::AxDomIdentifier,
        value: "compose".into(),
    });
    assert_eq!(
        native_identifier_attribute(IdentifierKind::AxDomIdentifier),
        Some("AXDOMIdentifier")
    );
    assert_eq!(
        token_mismatch(
            &identified,
            &facts(7, Some("AXButton"), None, Some("compose"))
        ),
        None
    );
    assert_eq!(
        token_mismatch(
            &identified,
            &facts(7, Some("AXButton"), None, Some("other"))
        ),
        Some(RetainedMismatch::NativeId)
    );
    assert_eq!(
        token_mismatch(&identified, &facts(7, Some("AXButton"), None, None)),
        Some(RetainedMismatch::NativeId)
    );

    assert_eq!(
        native_identifier_attribute(IdentifierKind::AutomationId),
        None
    );
    let mut unreadable = entry("button");
    unreadable.identity.native_id = Some(ElementIdentifier {
        kind: IdentifierKind::AutomationId,
        value: "save-button".into(),
    });
    assert_eq!(
        token_mismatch(&unreadable, &facts(7, Some("AXButton"), None, None)),
        None
    );
    let mut blank = entry("button");
    blank.identity.native_id = Some(ElementIdentifier {
        kind: IdentifierKind::AxIdentifier,
        value: "   ".into(),
    });
    assert_eq!(
        token_mismatch(&blank, &facts(7, Some("AXButton"), None, None)),
        None
    );
}

#[test]
fn a_ref_without_a_token_keeps_the_attribute_path() {
    let stored = entry("button");
    let deadline = Deadline::after(5000).unwrap();

    assert!(resolve_saved_token(&stored, deadline).unwrap().is_none());
}

#[test]
fn a_bare_token_saved_before_core_tagged_tokens_is_refused_not_resolved_by_fields() {
    let mut saved = entry("button");
    saved.identity.unique_at_capture = true;
    saved.identity.retained_object = Some("axtok1:00ff".into());
    let error = super::super::retained::refuse_untagged_token(&saved).expect_err("refused");
    assert_eq!(error.code, agent_desktop_core::ErrorCode::StaleRef);
    assert_eq!(error.details.as_ref().expect("details")["reason"], "legacy");

    saved.identity.retained_object = Some("token:axtok1:00ff".into());
    assert!(super::super::retained::refuse_untagged_token(&saved).is_ok());
}
