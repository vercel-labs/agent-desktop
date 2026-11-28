//! State machine for locating a named selection item over an abstract item source.
//! Each search phase is a named state, and `locate` is the only place that transitions between them.

use agent_desktop_core::AdapterError;

use crate::actions::chain::after_delivery;
use crate::actions::select_container::Realization;

pub(crate) const MAX_REALIZE_SCROLLS: u32 = 16;

/// One realize scroll attempt: either expose a searchable window or stop.
pub(crate) enum RealizeScrollStep {
    Window { end_of_range: bool },
    Stop,
}

/// What the search asks of the container it searches.
pub(crate) trait ItemSource {
    type Item: Clone;

    /// Walks the items the container has built; a match that is not `prior` is a second item and fails `AMBIGUOUS_TARGET`.
    fn scan(&mut self, prior: Option<&Self::Item>) -> Result<Option<Self::Item>, AdapterError>;
    /// The scroll position of the window `scan` reads, when the container reports one.
    fn position(&self) -> Option<f64>;
    /// Scrolls one step toward the end of the list.
    fn scroll_step(&mut self) -> Result<RealizeScrollStep, AdapterError>;
    /// Scrolls back to `position`; `false` when the container cannot.
    fn scroll_to(&mut self, position: f64) -> Result<bool, AdapterError>;
    /// Asks the container's own lookup for the item and builds it, reporting whether that sent a request.
    fn realize(&mut self) -> Result<Option<(Self::Item, Realization)>, AdapterError>;
}

/// The answer of the search.
pub(crate) enum Located<T> {
    /// The item, and whether building it sent a request to the application.
    Found {
        item: T,
        realized: bool,
    },
    Absent,
}

/// A matching item and the scroll position of the window it was seen in.
struct Sighting<T> {
    item: T,
    position: Option<f64>,
}

enum Phase<T> {
    FirstScan,
    ContainerLookup(Option<Sighting<T>>),
    Sweep(Option<Sighting<T>>),
    Rescan(Sighting<T>),
    Rebuild(Sighting<T>),
    Return(Sighting<T>),
    Done(Located<T>),
}

/// Finds the named item. The order is: scan the built items, ask the container's
/// own lookup (which proves the name unique itself), and only then sweep the
/// scroll range for a second same-name item. The scroll search takes no
/// foreground (A31-1, A31-2), so every policy runs it.
pub(crate) fn locate<S: ItemSource>(source: &mut S) -> Result<Located<S::Item>, AdapterError> {
    let mut phase = Phase::FirstScan;
    loop {
        phase = match phase {
            Phase::FirstScan => first_scan(source)?,
            Phase::ContainerLookup(sighting) => container_lookup(source, sighting)?,
            Phase::Sweep(sighting) => sweep(source, sighting)?,
            Phase::Rescan(sighting) => rescan(source, sighting)?,
            Phase::Rebuild(sighting) => rebuild(source, sighting)?,
            Phase::Return(sighting) => return_to(source, sighting)?,
            Phase::Done(located) => return Ok(located),
        };
    }
}

fn first_scan<S: ItemSource>(source: &mut S) -> Result<Phase<S::Item>, AdapterError> {
    let position = source.position();
    let sighting = source.scan(None)?.map(|item| Sighting { item, position });
    Ok(Phase::ContainerLookup(sighting))
}

fn container_lookup<S: ItemSource>(
    source: &mut S,
    sighting: Option<Sighting<S::Item>>,
) -> Result<Phase<S::Item>, AdapterError> {
    let Some((item, realization)) = source.realize()? else {
        return Ok(Phase::Sweep(sighting));
    };
    let realized = realization == Realization::Sent;
    let item = match sighting {
        Some(seen) => source
            .scan(Some(&seen.item))
            .map_err(|error| {
                if realized {
                    after_delivery(error)
                } else {
                    error
                }
            })?
            .unwrap_or(seen.item),
        None => item,
    };
    Ok(Phase::Done(Located::Found { item, realized }))
}

fn sweep<S: ItemSource>(
    source: &mut S,
    mut sighting: Option<Sighting<S::Item>>,
) -> Result<Phase<S::Item>, AdapterError> {
    let start = source.position();
    if let Some(seen) = sighting.take_if(|_| start.is_none()) {
        return Ok(Phase::Done(Located::Found {
            item: seen.item,
            realized: false,
        }));
    }
    let mut scrolled = false;
    for _ in 0..MAX_REALIZE_SCROLLS {
        match source.scroll_step()? {
            RealizeScrollStep::Stop => break,
            RealizeScrollStep::Window { end_of_range } => {
                scrolled = true;
                let position = source.position();
                let prior = sighting.as_ref().map(|seen| &seen.item);
                if let Some(item) = source.scan(prior)?
                    && sighting.is_none()
                {
                    sighting = Some(Sighting { item, position });
                }
                if end_of_range {
                    break;
                }
            }
        }
    }
    Ok(match sighting {
        Some(seen) if scrolled => Phase::Rescan(seen),
        Some(seen) => Phase::Done(Located::Found {
            item: seen.item,
            realized: false,
        }),
        None => {
            if let (true, Some(position)) = (scrolled, start) {
                source.scroll_to(position)?;
            }
            Phase::Done(Located::Absent)
        }
    })
}

fn rescan<S: ItemSource>(
    source: &mut S,
    sighting: Sighting<S::Item>,
) -> Result<Phase<S::Item>, AdapterError> {
    Ok(match source.scan(Some(&sighting.item))? {
        Some(item) => Phase::Done(Located::Found {
            item,
            realized: false,
        }),
        None => Phase::Rebuild(sighting),
    })
}

fn rebuild<S: ItemSource>(
    source: &mut S,
    sighting: Sighting<S::Item>,
) -> Result<Phase<S::Item>, AdapterError> {
    Ok(match source.realize()? {
        Some((item, realization)) => Phase::Done(Located::Found {
            item,
            realized: realization == Realization::Sent,
        }),
        None => Phase::Return(sighting),
    })
}

fn return_to<S: ItemSource>(
    source: &mut S,
    sighting: Sighting<S::Item>,
) -> Result<Phase<S::Item>, AdapterError> {
    let Some(position) = sighting.position else {
        return Ok(Phase::Done(Located::Absent));
    };
    if !source.scroll_to(position)? {
        return Ok(Phase::Done(Located::Absent));
    }
    Ok(match source.scan(Some(&sighting.item))? {
        Some(item) => Phase::Done(Located::Found {
            item,
            realized: false,
        }),
        None => Phase::Done(Located::Absent),
    })
}

#[cfg(test)]
#[path = "select_locate_tests.rs"]
mod tests;
