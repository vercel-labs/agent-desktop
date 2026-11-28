use super::*;
use agent_desktop_core::{
    EventKind, ProcessId, SignalBaseline, SignalCompleteness, SnapshotSurface, SurfaceSignal,
    diff_signals,
};

fn window(
    number: Option<i64>,
    children: Vec<ChildSurface>,
    popover: ObservedPopover,
) -> WindowObservation {
    WindowObservation {
        number,
        kind: None,
        title: Some("Document".into()),
        children,
        popover,
    }
}

fn sheet() -> ChildSurface {
    ChildSurface {
        number: None,
        kind: "sheet",
        title: Some("Sheet".into()),
        index: 0,
    }
}

fn ids(surfaces: &[SurfaceInfo]) -> Vec<&str> {
    surfaces.iter().map(|surface| surface.id.as_str()).collect()
}

#[test]
fn a_window_is_identified_by_its_ax_window_number() {
    let surfaces = window_surfaces(window(Some(42), vec![sheet()], ObservedPopover::Absent));
    assert_eq!(ids(&surfaces), ["w-42/sheet/0"]);
}

#[test]
fn a_surface_with_its_own_window_number_keeps_that_number() {
    let surfaces = window_surfaces(window(
        Some(42),
        vec![ChildSurface {
            number: Some(51),
            kind: "sheet",
            title: Some("Save".into()),
            index: 0,
        }],
        ObservedPopover::Found {
            number: Some(52),
            title: Some("Filters".into()),
        },
    ));
    assert_eq!(ids(&surfaces), ["w-51", "w-52"]);
}

#[test]
fn closing_an_earlier_window_never_re_ids_a_surviving_surface() {
    let before = [
        window(
            Some(11),
            vec![sheet()],
            ObservedPopover::Found {
                number: None,
                title: Some("Filters".into()),
            },
        ),
        window(
            Some(42),
            vec![sheet()],
            ObservedPopover::Found {
                number: None,
                title: Some("Options".into()),
            },
        ),
    ]
    .into_iter()
    .flat_map(window_surfaces)
    .collect::<Vec<_>>();
    let after = [window(
        Some(42),
        vec![sheet()],
        ObservedPopover::Found {
            number: None,
            title: Some("Options".into()),
        },
    )]
    .into_iter()
    .flat_map(window_surfaces)
    .collect::<Vec<_>>();

    assert_eq!(
        ids(&before),
        [
            "w-11/sheet/0",
            "w-11/popover",
            "w-42/sheet/0",
            "w-42/popover"
        ]
    );
    assert_eq!(ids(&after), ["w-42/sheet/0", "w-42/popover"]);

    let events = diff_signals(&baseline(before), &baseline(after));
    assert!(
        events
            .iter()
            .all(|event| !matches!(event.kind, EventKind::SurfaceAppeared { .. })),
        "closing the first window re-identified the second window's surfaces: {events:?}"
    );
    assert!(
        events
            .iter()
            .filter(|event| matches!(event.kind, EventKind::SurfaceDismissed { .. }))
            .count()
            == 2,
        "only the closed window's surfaces were dismissed: {events:?}"
    );
}

fn baseline(surfaces: Vec<SurfaceInfo>) -> SignalBaseline {
    SignalBaseline {
        windows: Vec::new(),
        apps: Vec::new(),
        surfaces: surfaces
            .into_iter()
            .filter_map(|surface| {
                Some(SurfaceSignal {
                    kind: kind_of_listed(&surface.kind)?,
                    app: "SurfaceApp".into(),
                    pid: ProcessId::new(42),
                    process_instance: "instance-42".into(),
                    id: surface.id,
                    title: surface.title,
                })
            })
            .collect(),
        completeness: SignalCompleteness::complete(),
    }
}

fn kind_of_listed(kind: &str) -> Option<SnapshotSurface> {
    match kind {
        "sheet" => Some(SnapshotSurface::Sheet),
        "popover" => Some(SnapshotSurface::Popover),
        "alert" => Some(SnapshotSurface::Alert),
        "menu" | "context_menu" => Some(SnapshotSurface::Menu),
        _ => None,
    }
}

#[test]
fn an_unnumbered_window_lists_a_surface_that_carries_its_own_number() {
    let surfaces = window_surfaces(window(
        None,
        vec![ChildSurface {
            number: Some(51),
            kind: "sheet",
            title: Some("Save".into()),
            index: 0,
        }],
        ObservedPopover::Absent,
    ));
    assert_eq!(ids(&surfaces), ["w-51"]);
}

#[test]
fn an_unnumbered_window_reports_only_the_kinds_it_could_not_identify() {
    let surfaces = window_surfaces(window(
        None,
        vec![sheet()],
        ObservedPopover::Unread {
            title: Some("Document".into()),
        },
    ));
    assert_eq!(ids(&surfaces), ["window/unread"]);
    assert_eq!(surfaces[0].kind, "window");
    assert_eq!(surfaces[0].unclassified, ["sheet", "popover"]);
    assert_eq!(surfaces[0].title, None);
}

#[test]
fn a_plain_unnumbered_window_lists_nothing() {
    let surfaces = window_surfaces(window(None, Vec::new(), ObservedPopover::Absent));
    assert!(surfaces.is_empty());
}

#[test]
fn a_cut_popover_search_names_popover_unread_on_the_window() {
    let surfaces = window_surfaces(window(
        Some(7),
        Vec::new(),
        ObservedPopover::Unread {
            title: Some("Document".into()),
        },
    ));
    assert_eq!(ids(&surfaces), ["w-7"]);
    assert_eq!(surfaces[0].kind, "window");
    assert_eq!(surfaces[0].unclassified, ["popover"]);

    let marked = window_surfaces(window(
        Some(7),
        Vec::new(),
        ObservedPopover::Unread { title: None },
    ));
    assert_eq!(marked[0].unclassified, ["popover"]);
}

#[test]
fn a_complete_popover_search_without_a_popover_lists_nothing() {
    let surfaces = window_surfaces(window(Some(7), Vec::new(), ObservedPopover::Absent));
    assert!(surfaces.is_empty());
}

#[test]
fn duplicate_reads_of_one_surface_are_listed_once() {
    let mut surfaces = vec![SurfaceInfo {
        id: "w-51".into(),
        kind: "sheet".into(),
        title: None,
        item_count: None,
        unclassified: Vec::new(),
    }];
    push_unique(
        &mut surfaces,
        SurfaceInfo {
            id: "w-51".into(),
            kind: "sheet".into(),
            title: None,
            item_count: None,
            unclassified: Vec::new(),
        },
    );
    push_unique(
        &mut surfaces,
        SurfaceInfo {
            id: "w-51".into(),
            kind: "popover".into(),
            title: None,
            item_count: None,
            unclassified: Vec::new(),
        },
    );
    assert_eq!(ids(&surfaces), ["w-51", "w-51"]);
    assert_eq!(surfaces[1].kind, "popover");
}
