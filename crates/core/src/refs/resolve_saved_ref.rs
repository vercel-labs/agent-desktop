use crate::{
    AdapterError, AppError, AppInfo, Deadline, IdentityMatch, IdentityProof, PlatformAdapter,
    RefEntry, SnapshotSurface, WindowInfo,
    adapter::{NativeHandle, TreeOptions},
    locator::live_locator::{ObservationRequest, ObservationRoot, ObservedTree},
    refs::ref_full_predicate::FullPredicate,
    refs::ref_identity::element_token,
    refs::ref_live_predicate::LivePredicate,
    refs::ref_scope_resolve::{chain_nodes, land_in_scope, match_in_scope},
    refs::resolve_errors::{absent, identity_twins, identity_unknown_error, identity_unproven},
};

#[path = "saved_ref_landing.rs"]
pub(crate) mod saved_ref_landing;

/// The depth a whole-window observation reaches: the limit a snapshot walk
/// supports, so a ref taken from the deepest snapshot node is inside the scope
/// it is captured from and resolved against.
pub(crate) const WHOLE_WINDOW_DEPTH: u8 = 50;

/// Resolves a saved ref under the reviewed identity rule.
///
/// A genuine retained element token resolves through the adapter, which owns
/// re-identification on that platform. Every other entry resolves only when it
/// was unique at capture and exactly one live element still matches its full
/// recorded predicate over complete evidence of the window it was captured
/// from; anything less is refused instead of guessed at.
pub fn resolve_saved_ref(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    deadline: Deadline,
    lease: Option<&crate::InteractionLease>,
) -> Result<NativeHandle, AdapterError> {
    deadline.ensure_remaining()?;
    if element_token(entry).is_some() {
        return adapter.resolve_element_strict(entry, deadline);
    }
    let (_, observed, found) = observe_saved_ref(adapter, entry, deadline, lease, true)?;
    land_in_scope(adapter, entry, &observed, found, deadline, lease)
}

/// Lands a saved ref at its recorded path and confirms it, skipping the
/// whole-window count when it can; see [`saved_ref_landing`].
pub(crate) fn land_saved_ref(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    deadline: Deadline,
) -> Result<NativeHandle, AdapterError> {
    saved_ref_landing::land_saved_ref_counted(adapter, entry, deadline).map(|(handle, _)| handle)
}

fn refuse_unless_unique(entry: &RefEntry) -> Result<(), AdapterError> {
    match entry.identity.proof {
        IdentityProof::Unique => Ok(()),
        IdentityProof::Twins => Err(identity_twins(entry)),
        IdentityProof::Unproven => Err(identity_unproven(entry)),
    }
}

/// The window a saved ref is captured from, one complete observation of it, and
/// the node in that observation it names.
pub(crate) type SavedRefMatch = (WindowInfo, ObservedTree, usize);

/// The evidence every tokenless ref is judged by, shared by resolution, by a
/// drill-down and by a rooted find, so a capture proof and a later resolve
/// measure one observation. A ref carrying an element token names its node by
/// that token; every other ref names it by the predicate it was capture-proven
/// with, and an entry that was not unique at capture has no such evidence to
/// gather. `identity_only` marks a read that only resolves and publishes nothing.
pub(crate) fn observe_saved_ref(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    deadline: Deadline,
    lease: Option<&crate::InteractionLease>,
    identity_only: bool,
) -> Result<SavedRefMatch, AdapterError> {
    let token = element_token(entry);
    if token.is_none() {
        refuse_unless_unique(entry)?;
    }
    let window = recorded_window(adapter, entry, deadline)?;
    let base = TreeOptions {
        surface: entry.source.source_surface,
        force_renderer_accessibility: entry.source.renderer_accessibility_forced,
        ..TreeOptions::default()
    };
    let mut request = whole_window_request(&base, deadline)?;
    request.observation_mode.identity_only = identity_only;
    let observed = observe_window(
        adapter,
        &request,
        &window,
        lease,
        entry.source.source_window_id.is_some(),
    )?;
    let index = saved_ref_node(entry, token, &observed)?;
    let node = observed
        .nodes
        .get(index)
        .ok_or_else(|| AdapterError::internal("saved ref match is missing from the observation"))?;
    adapter.validate_saved_ref_context(entry, node.evidence.identifiers.retained_object())?;
    Ok((window, observed, index))
}

/// Confirms that the element an adapter landed is the one `entry` names, under
/// the same full predicate a saved ref resolves by. A caller that publishes a
/// landed handle - the C find entry point hands one to C callers - validates it
/// here instead of trusting the landing alone.
pub fn confirm_landed_element(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    handle: &NativeHandle,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    let root = ObservationRoot::Element {
        handle,
        entry,
        root_ref: None,
    };
    confirm_landed(adapter, entry, root, deadline, None)
}

/// The anchor lands a stored path, which a different element can move into
/// between the live count and the descent, so the handle is re-identified
/// before it is handed back: only the recorded predicate proves the landing.
pub(crate) fn confirm_landed(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    root: ObservationRoot<'_>,
    deadline: Deadline,
    lease: Option<&crate::InteractionLease>,
) -> Result<(), AdapterError> {
    let options = TreeOptions {
        max_depth: 0,
        surface: entry.source.source_surface,
        ..TreeOptions::default()
    };
    let request = ObservationRequest::snapshot(&options, deadline).validate()?;
    let observed =
        crate::tree::renderer_accessibility::observe_tree(adapter, root, &request, lease)
            .map_err(AppError::into_adapter_error)?;
    let landed = observed
        .roots
        .first()
        .and_then(|index| observed.nodes.get(*index as usize))
        .ok_or_else(|| AdapterError::internal("the landed element was not observed"))?;
    match FullPredicate::of_entry(entry).compare(&LivePredicate::observed(&landed.evidence)) {
        IdentityMatch::Match => Ok(()),
        IdentityMatch::Unknown => Err(identity_unknown_error(entry)),
        IdentityMatch::NoMatch => Err(absent(entry)),
    }
}

/// The window the ref was captured from, never another window of the process;
/// a process-owned ref's owner is its live process, whatever windows it opens.
/// The surface lookup resolves shell surfaces, the focused window and a recorded
/// window id; an entry with no window id falls back to the process's windows,
/// where the generation says which one it was.
pub(crate) fn recorded_window(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    deadline: Deadline,
) -> Result<WindowInfo, AdapterError> {
    if entry.source.is_process_owned() {
        return crate::system::app_lookup::live_process_owner(adapter, entry, deadline)
            .map_err(AppError::into_adapter_error);
    }
    if let Some(window_id) = entry.source.source_window_id.as_deref()
        && let Some(process) = entry.process.identity()
        && let Some(window) = adapter.recorded_surface_window(
            process,
            window_id,
            entry.source.source_surface,
            deadline,
        )?
    {
        return Ok(window);
    }
    let resolved = crate::snapshot::resolve_window_for_surface(
        adapter,
        entry.source.source_app.as_deref(),
        entry.source.source_window_id.as_deref(),
        entry.source.source_surface,
        deadline,
    );
    let mut failure = None;
    match resolved.map_err(AppError::into_adapter_error) {
        Ok(window) if belongs_to_entry(&window, entry) => return Ok(window),
        Ok(_) => {}
        Err(error) if error.code == crate::ErrorCode::WindowNotFound => {}
        Err(error) => failure = Some(error),
    }
    if entry.source.source_window_id.is_some() {
        return Err(failure.unwrap_or_else(gone));
    }
    let listed = adapter.list_windows(
        &crate::adapter::WindowFilter {
            focused_only: false,
            app: entry.source.source_app.clone(),
        },
        deadline,
    )?;
    listed
        .into_iter()
        .find(|window| belongs_to_entry(window, entry))
        .ok_or_else(|| failure.unwrap_or_else(gone))
}

fn gone() -> AdapterError {
    AdapterError::stale_ref_because("The window the ref was captured from is gone")
}

/// Whether a live window is the one the entry was captured from: the owner kind,
/// pid and a recorded window id must agree, and process generations must agree
/// when both sides carry one, since some macOS window records omit it.
fn belongs_to_entry(window: &WindowInfo, entry: &RefEntry) -> bool {
    window.is_process_owner() == entry.source.is_process_owned()
        && window.pid == entry.process.pid
        && entry
            .source
            .source_window_id
            .as_deref()
            .is_none_or(|id| window.id == id)
        && window
            .process_instance
            .as_deref()
            .zip(entry.process.process_instance.as_deref())
            .is_none_or(|(live, saved)| live == saved)
}

/// The one observation a tokenless ref is judged by, in the settings resolution
/// uses: the whole window at the deepest limit a walk supports, so a capture
/// proof and a later resolution count the same evidence. `base` supplies the
/// surface and the renderer-accessibility hint; its depth and presentation
/// settings are ignored. `window` is the owner the capture resolved, so a
/// surface presented by that window is read at it.
pub(crate) fn observe_whole_window(
    adapter: &dyn PlatformAdapter,
    base: &TreeOptions,
    window: &WindowInfo,
    deadline: Deadline,
) -> Result<ObservedTree, AdapterError> {
    observe_window(
        adapter,
        &whole_window_request(base, deadline)?,
        window,
        None,
        true,
    )
}

fn whole_window_request(
    base: &TreeOptions,
    deadline: Deadline,
) -> Result<ObservationRequest, AdapterError> {
    let options = TreeOptions {
        max_depth: WHOLE_WINDOW_DEPTH,
        surface: base.surface,
        force_renderer_accessibility: base.force_renderer_accessibility,
        ..TreeOptions::default()
    }
    .with_ref_identity_bounds();
    ObservationRequest::snapshot(&options, deadline).validate()
}

fn observe_window(
    adapter: &dyn PlatformAdapter,
    request: &ObservationRequest,
    window: &WindowInfo,
    lease: Option<&crate::InteractionLease>,
    surface_window: bool,
) -> Result<ObservedTree, AdapterError> {
    let process_instance = window.process_instance.clone();
    let app = AppInfo {
        aliases: Vec::new(),
        name: window.app.clone(),
        pid: window.pid,
        process_instance,
        bundle_id: None,
        presentation: None,
    };
    let root = observation_root(adapter, request.surface, &app, window, surface_window);
    let mut observed =
        crate::tree::renderer_accessibility::observe_tree(adapter, root, request, lease)
            .map_err(AppError::into_adapter_error)?;
    if let crate::locator::live_locator::ObservationSource::Window {
        renderer_accessibility_forced,
        ..
    } = &mut observed.source
    {
        *renderer_accessibility_forced = request.observation_mode.force_renderer_accessibility;
    }
    Ok(observed)
}

/// The root one observation of `surface` starts at. `surface_window` says the
/// window is the one the surface is presented by - the id a ref recorded, or the
/// owner a capture resolved - so the read lands on that window instead of
/// re-resolving the surface from the process, which cannot name it while several
/// of that kind are open. Every other application-owned surface has no window of
/// its own and starts at the application root.
fn observation_root<'a>(
    adapter: &dyn PlatformAdapter,
    surface: SnapshotSurface,
    app: &'a AppInfo,
    window: &'a WindowInfo,
    surface_window: bool,
) -> ObservationRoot<'a> {
    let presented_by_window = surface_window && !window.is_process_owner();
    if adapter.is_application_surface(surface) && !presented_by_window {
        ObservationRoot::Application { app, surface }
    } else {
        ObservationRoot::Window(window)
    }
}

fn saved_ref_node(
    entry: &RefEntry,
    token: Option<&str>,
    observed: &ObservedTree,
) -> Result<usize, AdapterError> {
    if let Some(token) = token {
        let found = observed
            .nodes
            .iter()
            .position(|node| node.evidence.identifiers.element_token() == Some(token));
        return match found {
            Some(index) => Ok(index),
            None if observed.is_exhaustive() => Err(absent(entry)),
            None => Err(identity_unknown_error(entry)),
        };
    }
    unique_live_match(entry, observed)
}

fn unique_live_match(entry: &RefEntry, observed: &ObservedTree) -> Result<usize, AdapterError> {
    let scope = chain_nodes(observed, &entry.scope.chain)
        .map_err(|miss| miss.into_error(entry))?
        .last()
        .copied();
    match_in_scope(observed, scope, &FullPredicate::of_entry(entry))
        .map_err(|miss| miss.into_error(entry))
}

#[cfg(test)]
#[path = "resolve_saved_ref_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "resolve_saved_ref_window_tests.rs"]
mod window_tests;

#[cfg(test)]
#[path = "resolve_saved_ref_context_tests.rs"]
mod context_tests;

#[cfg(test)]
#[path = "resolve_saved_ref_scope_tests.rs"]
mod scope_tests;

#[cfg(test)]
#[path = "resolve_saved_ref_scope_find_tests.rs"]
mod scope_find_tests;

#[cfg(test)]
#[path = "resolve_saved_ref_label_tests.rs"]
mod label_tests;

#[cfg(test)]
#[path = "resolve_saved_ref_test_support.rs"]
mod test_support;
