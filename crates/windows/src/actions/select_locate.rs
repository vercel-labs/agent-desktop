//! State machine for locating a named selection item over an abstract item source.
//! Each search phase is a named state, and `locate` is the only place that transitions between them.

use agent_desktop_core::AdapterError;

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
    /// Whether a vertical scroll can hide items from `scan`.
    fn hides_items_behind_a_scroll(&self) -> bool;
    /// The scroll position of the window `scan` reads, when the container reports one.
    fn position(&self) -> Option<f64>;
    /// Scrolls one step toward the end of the list.
    fn scroll_step(&mut self) -> Result<RealizeScrollStep, AdapterError>;
    /// Scrolls back to `position`; `false` when the container cannot.
    fn scroll_to(&mut self, position: f64) -> Result<bool, AdapterError>;
    /// Asks the container's own lookup for the item and builds it.
    fn realize(&mut self) -> Result<Option<Self::Item>, AdapterError>;
}

/// The answer of the search.
pub(crate) enum Located<T> {
    /// The item, and whether building it sent a request to the application.
    Found {
        item: T,
        realized: bool,
    },
    Absent,
    /// Nothing matched and the scroll that could show more items is not allowed.
    ScrollDenied,
}

/// A matching item and the scroll position of the window it was seen in.
struct Sighting<T> {
    item: T,
    position: Option<f64>,
}

enum Phase<T> {
    FirstScan,
    ContainerLookup,
    Sweep(Option<Sighting<T>>),
    Rescan(Sighting<T>),
    Rebuild(Sighting<T>),
    Return(Sighting<T>),
    Done(Located<T>),
}

/// Finds the named item. `may_scroll` is whether the policy allows calls that change what the user sees.
pub(crate) fn locate<S: ItemSource>(
    source: &mut S,
    may_scroll: bool,
) -> Result<Located<S::Item>, AdapterError> {
    let mut phase = Phase::FirstScan;
    loop {
        phase = match phase {
            Phase::FirstScan => first_scan(source, may_scroll)?,
            Phase::ContainerLookup => container_lookup(source)?,
            Phase::Sweep(sighting) => sweep(source, sighting)?,
            Phase::Rescan(sighting) => rescan(source, sighting)?,
            Phase::Rebuild(sighting) => rebuild(source, sighting)?,
            Phase::Return(sighting) => return_to(source, sighting)?,
            Phase::Done(located) => return Ok(located),
        };
    }
}

fn first_scan<S: ItemSource>(
    source: &mut S,
    may_scroll: bool,
) -> Result<Phase<S::Item>, AdapterError> {
    let position = source.position();
    if let Some(item) = source.scan(None)? {
        return Ok(if may_scroll {
            Phase::Sweep(Some(Sighting { item, position }))
        } else {
            Phase::Done(Located::Found {
                item,
                realized: false,
            })
        });
    }
    if may_scroll {
        return Ok(Phase::ContainerLookup);
    }
    if source.hides_items_behind_a_scroll() {
        return Ok(Phase::Done(Located::ScrollDenied));
    }
    Ok(Phase::Done(Located::Absent))
}

fn container_lookup<S: ItemSource>(source: &mut S) -> Result<Phase<S::Item>, AdapterError> {
    Ok(match source.realize()? {
        Some(item) => Phase::Done(Located::Found {
            item,
            realized: true,
        }),
        None => Phase::Sweep(None),
    })
}

fn sweep<S: ItemSource>(
    source: &mut S,
    mut sighting: Option<Sighting<S::Item>>,
) -> Result<Phase<S::Item>, AdapterError> {
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
        None => Phase::Done(Located::Absent),
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
        Some(item) => Phase::Done(Located::Found {
            item,
            realized: true,
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
