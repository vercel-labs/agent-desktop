use crate::{
    AdapterError, AppError, Deadline, LocatorQuery, PlatformAdapter, RefEntry, SnapshotOwner,
    SnapshotSurface,
    locator::live_locator::{
        LocatorMaterialization, LocatorResolveRequest, LocatorSelection, incomplete_locator_error,
        resolve_query,
    },
    snapshot::{ref_owner_observation::RefOwnerObservation, surface_target, window_capture},
    system::process_state::ProcessState,
};

pub(crate) enum SelectorObservation {
    Selector {
        matches: bool,
        incomplete: Option<AdapterError>,
    },
    OwnerUnavailable(ProcessState),
}

impl SelectorObservation {
    pub(crate) fn observe_ref(
        adapter: &dyn PlatformAdapter,
        entry: &RefEntry,
        query: &LocatorQuery,
        deadline: Deadline,
    ) -> Result<Self, AppError> {
        match surface_target::resolve_ref_owner(adapter, entry, deadline)? {
            RefOwnerObservation::Present(owner) => Self::observe_owners(
                adapter,
                vec![owner],
                surface_target::ref_observation_surface(adapter, entry),
                query,
                deadline,
            ),
            RefOwnerObservation::Absent(None) => Ok(Self::Selector {
                matches: false,
                incomplete: None,
            }),
            RefOwnerObservation::Absent(Some(state)) | RefOwnerObservation::Unavailable(state) => {
                Ok(Self::OwnerUnavailable(state))
            }
        }
    }

    pub(crate) fn observe_owners(
        adapter: &dyn PlatformAdapter,
        owners: Vec<SnapshotOwner>,
        surface: SnapshotSurface,
        query: &LocatorQuery,
        deadline: Deadline,
    ) -> Result<Self, AppError> {
        if owners.is_empty() {
            return Ok(Self::Selector {
                matches: false,
                incomplete: None,
            });
        }
        let request = LocatorResolveRequest {
            selection: LocatorSelection::First,
            deadline,
            max_raw_depth: 50,
            surface: (surface != SnapshotSurface::Window).then_some(surface),
            materialization: LocatorMaterialization::None,
        };
        match window_capture::first_surface_owner(owners, |owner| {
            resolve_query(adapter, query, owner.observation_root(surface), &request)
        }) {
            Ok((_, resolution)) => Ok(Self::Selector {
                matches: resolution.meta.total_matches > 0,
                incomplete: (!resolution.meta.selection_complete)
                    .then(|| incomplete_locator_error(&resolution)),
            }),
            Err(error) if surface_target::is_confirmed_surface_absence(&error, surface) => {
                Ok(Self::Selector {
                    matches: false,
                    incomplete: None,
                })
            }
            Err(error) => Err(error),
        }
    }
}
