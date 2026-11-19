//! A loaded custom cursor image, shared between the cursor that shows it and
//! the window that draws it.
use std::rc::Rc;

use agent_desktop_core::Point;

use super::raster::Surface;

/// The scaled image and the point of it that sits on the tip, in the
/// unscaled image pixels the request gives. Two pictures are the same only
/// when they are the same load, so comparing them never walks the pixels.
#[derive(Debug, Clone)]
pub(crate) struct Picture(Rc<(Surface, Point)>);

impl Picture {
    pub(crate) fn new(surface: Surface, hotspot: Point) -> Self {
        Self(Rc::new((surface, hotspot)))
    }

    pub(crate) fn surface(&self) -> &Surface {
        &self.0.0
    }

    pub(crate) fn hotspot(&self) -> &Point {
        &self.0.1
    }
}

impl PartialEq for Picture {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
