//! A style's custom cursor images, loaded once for each style and scale. An
//! image that cannot be loaded leaves the dart in its place.
use super::image;
use super::picture::Picture;
use super::request::{Face, Look};

type FaceKey = (Option<Face>, Option<Face>, u64);

#[derive(Debug, Clone, Default)]
pub(crate) struct Faces {
    key: Option<FaceKey>,
    arrow: Option<Picture>,
    pointer: Option<Picture>,
}

fn load(face: Option<&Face>, scale: f64) -> Option<Picture> {
    let face = face?;
    let surface = image::scaled(&image::load(&face.path)?, scale)?;
    Some(Picture::new(surface, face.hotspot.clone()))
}

impl Faces {
    /// Loads the images `look` names, unless they are already loaded at its
    /// size.
    pub(crate) fn restyle(&mut self, look: &Look) {
        let key = (
            look.arrow.clone(),
            look.pointer.clone(),
            look.size.to_bits(),
        );
        if self.key.as_ref() == Some(&key) {
            return;
        }
        self.arrow = load(look.arrow.as_ref(), look.size);
        self.pointer = load(look.pointer.as_ref(), look.size);
        self.key = Some(key);
    }

    /// The image to show: the pointer image while pointing, when there is
    /// one, and the arrow image otherwise. `None` draws the dart.
    pub(crate) fn face(&self, pointing: bool) -> Option<&Picture> {
        match (&self.pointer, pointing) {
            (Some(pointer), true) => Some(pointer),
            _ => self.arrow.as_ref(),
        }
    }
}
