use std::cell::RefCell;

use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode, ProcessIdentity, WindowInfo};

use super::{TransientSurface, close_with};

struct Lookups {
    window_surface: Option<&'static str>,
    window_error: Option<AdapterError>,
    process_surface: Option<&'static str>,
    process_error: Option<AdapterError>,
}

struct Guards {
    surface_error: Option<AdapterError>,
    generation_limit: Option<usize>,
    cancel_supported: bool,
    closed: bool,
}

struct Recorder {
    looked_in: RefCell<Vec<String>>,
    cancelled: RefCell<Option<&'static str>>,
    generations: RefCell<usize>,
}

struct FakeSurfaces {
    lookups: Lookups,
    guards: Guards,
    recorder: Recorder,
}

impl TransientSurface for FakeSurfaces {
    type Identity = u32;
    type Surface = &'static str;

    fn verify_process(&self, process: &ProcessIdentity) -> Result<u32, AdapterError> {
        match &self.lookups.process_error {
            Some(error) => Err(error.clone()),
            None => Ok(process.pid.get()),
        }
    }

    fn require_generation(&self, _identity: u32) -> Result<(), AdapterError> {
        let mut checks = self.recorder.generations.borrow_mut();
        *checks += 1;
        let limit = self.guards.generation_limit;
        if limit.is_some_and(|limit| *checks > limit) {
            return Err(not_delivered(AdapterError::stale_ref_because(
                "Application process instance changed during observation",
            )));
        }
        Ok(())
    }

    fn in_window(
        &self,
        _identity: u32,
        window: &WindowInfo,
    ) -> Result<Option<&'static str>, AdapterError> {
        self.recorder
            .looked_in
            .borrow_mut()
            .push(format!("window:{}", window.id));
        match &self.lookups.window_error {
            Some(error) => Err(error.clone()),
            None => Ok(self.lookups.window_surface),
        }
    }

    fn in_process(&self, identity: u32) -> Result<Option<&'static str>, AdapterError> {
        self.recorder
            .looked_in
            .borrow_mut()
            .push(format!("process:{identity}"));
        Ok(self.lookups.process_surface)
    }

    fn verify_surface(&self, identity: u32, _surface: &&'static str) -> Result<(), AdapterError> {
        self.require_generation(identity)?;
        match &self.guards.surface_error {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }

    fn cancel(&self, surface: &&'static str) -> Result<bool, AdapterError> {
        *self.recorder.cancelled.borrow_mut() = Some(*surface);
        Ok(self.guards.cancel_supported)
    }

    fn closed(&self, _surface: &&'static str) -> bool {
        self.guards.closed
    }
}

fn surfaces(surface: Option<&'static str>) -> FakeSurfaces {
    surfaces_split(surface, surface)
}

fn surfaces_split(
    window_surface: Option<&'static str>,
    process_surface: Option<&'static str>,
) -> FakeSurfaces {
    FakeSurfaces {
        lookups: Lookups {
            window_surface,
            window_error: None,
            process_surface,
            process_error: None,
        },
        guards: Guards {
            surface_error: None,
            generation_limit: None,
            cancel_supported: true,
            closed: true,
        },
        recorder: Recorder {
            looked_in: RefCell::new(Vec::new()),
            cancelled: RefCell::new(None),
            generations: RefCell::new(0),
        },
    }
}

fn not_delivered(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::not_delivered())
}

fn process() -> ProcessIdentity {
    ProcessIdentity::new(42_u32, "instance")
}

fn window() -> WindowInfo {
    WindowInfo {
        id: "W1".into(),
        title: "Main".into(),
        app: "Sample".into(),
        pid: 42_u32.into(),
        process_instance: Some("instance".into()),
        bounds: None,
        state: Default::default(),
    }
}

#[test]
fn an_app_scoped_close_searches_the_process_and_verifies_the_surface_gone() {
    let surfaces = surfaces(Some("popover"));

    let result = close_with(&surfaces, &process(), None)
        .unwrap()
        .expect("a closed surface");

    assert_eq!(*surfaces.recorder.looked_in.borrow(), ["process:42"]);
    assert_eq!(*surfaces.recorder.cancelled.borrow(), Some("popover"));
    assert_eq!(
        result.disposition(),
        DeliverySemantics::delivered_verified()
    );
    assert_eq!(result.steps[0].verified, Some(true));
}

#[test]
fn a_window_scoped_close_prefers_the_surface_in_that_window() {
    let surfaces = surfaces_split(Some("menu"), Some("app-menu"));

    let result = close_with(&surfaces, &process(), Some(&window()))
        .unwrap()
        .expect("a closed surface");

    assert_eq!(*surfaces.recorder.looked_in.borrow(), ["window:W1"]);
    assert_eq!(*surfaces.recorder.cancelled.borrow(), Some("menu"));
    assert_eq!(
        result.disposition(),
        DeliverySemantics::delivered_verified()
    );
}

#[test]
fn a_window_scoped_close_falls_back_to_the_surface_the_app_shows() {
    let surfaces = surfaces_split(None, Some("app-menu"));

    let result = close_with(&surfaces, &process(), Some(&window()))
        .unwrap()
        .expect("a closed surface");

    assert_eq!(
        *surfaces.recorder.looked_in.borrow(),
        ["window:W1", "process:42"]
    );
    assert_eq!(*surfaces.recorder.cancelled.borrow(), Some("app-menu"));
    assert_eq!(
        result.disposition(),
        DeliverySemantics::delivered_verified()
    );
    assert_eq!(result.steps[0].verified, Some(true));
}

#[test]
fn a_window_lookup_error_is_not_masked_by_the_app_fallback() {
    let mut surfaces = surfaces_split(Some("menu"), Some("app-menu"));
    surfaces.lookups.window_error = Some(AdapterError::new(
        ErrorCode::WindowNotFound,
        "Window closed",
    ));

    let error = close_with(&surfaces, &process(), Some(&window())).unwrap_err();

    assert_eq!(error.code, ErrorCode::WindowNotFound);
    assert_eq!(*surfaces.recorder.looked_in.borrow(), ["window:W1"]);
    assert_eq!(*surfaces.recorder.cancelled.borrow(), None);
}

#[test]
fn a_window_that_does_not_name_the_process_instance_fails_before_any_search() {
    for instance in [None, Some("other-launch".to_owned())] {
        let surfaces = surfaces_split(Some("menu"), Some("app-menu"));
        let mut window = window();
        window.process_instance = instance.clone();

        let error = close_with(&surfaces, &process(), Some(&window)).unwrap_err();

        assert_eq!(error.code, ErrorCode::StaleRef, "{instance:?}");
        assert_eq!(error.disposition, DeliverySemantics::not_delivered());
        assert!(
            surfaces.recorder.looked_in.borrow().is_empty(),
            "{instance:?}"
        );
        assert_eq!(*surfaces.recorder.cancelled.borrow(), None);
    }
}

#[test]
fn a_window_of_another_process_fails_before_any_search() {
    let surfaces = surfaces_split(Some("menu"), Some("app-menu"));
    let mut window = window();
    window.pid = 7_u32.into();

    let error = close_with(&surfaces, &process(), Some(&window)).unwrap_err();

    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert!(surfaces.recorder.looked_in.borrow().is_empty());
    assert_eq!(*surfaces.recorder.cancelled.borrow(), None);
}

#[test]
fn a_process_that_no_longer_matches_fails_before_any_search() {
    let mut surfaces = surfaces(Some("menu"));
    surfaces.lookups.process_error = Some(not_delivered(AdapterError::stale_ref_because(
        "Target process instance is no longer running",
    )));

    let error = close_with(&surfaces, &process(), None).unwrap_err();

    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert!(surfaces.recorder.looked_in.borrow().is_empty());
    assert_eq!(*surfaces.recorder.cancelled.borrow(), None);
}

#[test]
fn a_generation_change_before_the_app_fallback_never_searches_the_process() {
    let mut surfaces = surfaces_split(None, Some("app-menu"));
    surfaces.guards.generation_limit = Some(0);

    let error = close_with(&surfaces, &process(), Some(&window())).unwrap_err();

    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(*surfaces.recorder.looked_in.borrow(), ["window:W1"]);
    assert_eq!(*surfaces.recorder.cancelled.borrow(), None);
}

#[test]
fn a_generation_change_before_the_cancel_fails_without_cancelling() {
    let mut surfaces = surfaces(Some("menu"));
    surfaces.guards.generation_limit = Some(0);

    let error = close_with(&surfaces, &process(), None).unwrap_err();

    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(*surfaces.recorder.looked_in.borrow(), ["process:42"]);
    assert_eq!(*surfaces.recorder.cancelled.borrow(), None);
}

#[test]
fn a_surface_of_another_process_is_never_cancelled() {
    let mut surfaces = surfaces(Some("menu"));
    surfaces.guards.surface_error = Some(not_delivered(AdapterError::stale_ref_because(
        "Surface no longer belongs to the target process",
    )));

    let error = close_with(&surfaces, &process(), None).unwrap_err();

    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(*surfaces.recorder.looked_in.borrow(), ["process:42"]);
    assert_eq!(*surfaces.recorder.cancelled.borrow(), None);
}

#[test]
fn no_open_surface_leaves_the_refusal() {
    let surfaces = surfaces(None);

    assert!(close_with(&surfaces, &process(), None).unwrap().is_none());
    assert_eq!(*surfaces.recorder.looked_in.borrow(), ["process:42"]);
    assert_eq!(*surfaces.recorder.cancelled.borrow(), None);
}

#[test]
fn a_surface_that_does_not_take_the_cancel_leaves_the_refusal() {
    let mut surfaces = surfaces(Some("menu"));
    surfaces.guards.cancel_supported = false;

    assert!(close_with(&surfaces, &process(), None).unwrap().is_none());
    assert_eq!(*surfaces.recorder.cancelled.borrow(), Some("menu"));
}

#[test]
fn a_close_that_cannot_be_read_back_is_unverified() {
    let mut surfaces = surfaces(Some("menu"));
    surfaces.guards.closed = false;

    let result = close_with(&surfaces, &process(), None)
        .unwrap()
        .expect("a dispatched close");

    assert_eq!(
        result.disposition(),
        DeliverySemantics::delivered_unverified()
    );
    assert_eq!(result.steps[0].verified, Some(false));
}
