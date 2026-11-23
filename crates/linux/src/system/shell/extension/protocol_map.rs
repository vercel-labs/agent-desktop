//! How the X11 coordinates an Xwayland client reports land on the GNOME Shell
//! stage. With a fractional monitor scale, Xwayland can scale its clients, so
//! the screen positions their toolkits report are not the stage's logical
//! coordinates (`probes/linux/FINDINGS.md` L43-9).
use crate::tree::Extents;

/// One Xwayland window's frame in X11 coordinates and on the stage. Any other
/// X11 rectangle maps onto the stage by the same offset and scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProtocolMap {
    protocol: Extents,
    stage: Extents,
}

/// `value`, measured from `from_origin` on an axis `from_length` long, moved
/// onto the same point of an axis that starts at `to_origin` and is
/// `to_length` long, rounded to the nearest whole unit.
fn rescale(
    value: i32,
    from_origin: i32,
    from_length: i32,
    to_origin: i32,
    to_length: i32,
) -> Option<i32> {
    let offset = i64::from(value) - i64::from(from_origin);
    let doubled = offset.checked_mul(2)?.checked_mul(i64::from(to_length))?;
    let scaled = (doubled + i64::from(from_length)).div_euclid(2 * i64::from(from_length));
    i32::try_from(i64::from(to_origin).checked_add(scaled)?).ok()
}

impl ProtocolMap {
    /// The map from `protocol` to `stage`, two readings of one frame. `None`
    /// when either is empty, because an empty frame gives no scale.
    pub(crate) fn new(protocol: Extents, stage: Extents) -> Option<Self> {
        (protocol.width > 0 && protocol.height > 0 && stage.width > 0 && stage.height > 0)
            .then_some(Self { protocol, stage })
    }

    /// `rect`, in X11 coordinates, on the stage. Both edges are mapped, so
    /// rectangles that touch in X11 still touch on the stage.
    pub(crate) fn to_stage(self, rect: Extents) -> Option<Extents> {
        let (from, to) = (self.protocol, self.stage);
        let x = rescale(rect.x, from.x, from.width, to.x, to.width)?;
        let y = rescale(rect.y, from.y, from.height, to.y, to.height)?;
        let right = rescale(
            rect.x.checked_add(rect.width)?,
            from.x,
            from.width,
            to.x,
            to.width,
        )?;
        let bottom = rescale(
            rect.y.checked_add(rect.height)?,
            from.y,
            from.height,
            to.y,
            to.height,
        )?;
        Some(Extents {
            x,
            y,
            width: right.checked_sub(x)?,
            height: bottom.checked_sub(y)?,
        })
    }

    /// Whether `other` maps every rectangle the way this map does.
    pub(crate) fn agrees(self, other: Self) -> bool {
        self.to_stage(other.protocol) == Some(other.stage)
            && other.to_stage(self.protocol) == Some(self.stage)
    }
}

#[cfg(test)]
#[path = "protocol_map_tests.rs"]
mod tests;
