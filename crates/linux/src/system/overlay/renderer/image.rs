//! A custom cursor image: a PNG within core's limits, decoded to premultiplied
//! pixels and scaled by the style's size. A file that is missing, too large or
//! not a PNG gives `None`, and the dart is drawn instead, as the GNOME Shell
//! extension does.
use std::io::Read;

use agent_desktop_core::{MAX_CURSOR_IMAGE_BYTES, MAX_CURSOR_IMAGE_PIXELS};
use png::{ColorType, Decoder, Transformations};

use super::raster::Surface;

/// A decoded image at its own size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CursorPicture {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) pixels: Vec<u32>,
}

fn premultiplied(sample: &[u8]) -> u32 {
    let (red, green, blue, alpha) = match *sample {
        [r, g, b, a] => (r, g, b, a),
        [r, g, b] => (r, g, b, 255),
        [l, a] => (l, l, l, a),
        [l] => (l, l, l, 255),
        _ => (0, 0, 0, 0),
    };
    let scale = |channel: u8| (u32::from(channel) * u32::from(alpha) + 127) / 255;
    (u32::from(alpha) << 24) | (scale(red) << 16) | (scale(green) << 8) | scale(blue)
}

/// The PNG in `bytes`, or `None` when it is not one or exceeds core's limits.
pub(crate) fn decode(bytes: &[u8]) -> Option<CursorPicture> {
    if bytes.len() as u64 > MAX_CURSOR_IMAGE_BYTES {
        return None;
    }
    let mut decoder = Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(Transformations::EXPAND | Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let (width, height) = (reader.info().width, reader.info().height);
    if width == 0
        || height == 0
        || width > MAX_CURSOR_IMAGE_PIXELS
        || height > MAX_CURSOR_IMAGE_PIXELS
    {
        return None;
    }
    let mut frame = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut frame).ok()?;
    let channels = match info.color_type {
        ColorType::Rgba => 4,
        ColorType::Rgb => 3,
        ColorType::GrayscaleAlpha => 2,
        ColorType::Grayscale => 1,
        ColorType::Indexed => return None,
    };
    let count = (info.width as usize).checked_mul(info.height as usize)?;
    let pixels = frame
        .chunks_exact(channels)
        .take(count)
        .map(premultiplied)
        .collect::<Vec<_>>();
    (pixels.len() == count).then_some(CursorPicture {
        width: info.width,
        height: info.height,
        pixels,
    })
}

/// The image file at `path`, read within core's size limit.
pub(crate) fn load(path: &str) -> Option<CursorPicture> {
    let file = std::fs::File::open(path).ok()?;
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(MAX_CURSOR_IMAGE_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    decode(&bytes)
}

/// `picture` scaled by `scale` with nearest-neighbour sampling, as St scales a
/// background image.
pub(crate) fn scaled(picture: &CursorPicture, scale: f64) -> Option<Surface> {
    let length = |side: u32| {
        let length = (f64::from(side) * scale).ceil();
        (length >= 1.0 && length <= f64::from(u16::MAX)).then_some(length as u16)
    };
    let mut surface = Surface::transparent(length(picture.width)?, length(picture.height)?)?;
    let source = |target: u16, limit: u32| {
        ((f64::from(target) + 0.5) / scale)
            .floor()
            .clamp(0.0, f64::from(limit - 1)) as usize
    };
    for y in 0..surface.height {
        for x in 0..surface.width {
            let from =
                source(y, picture.height) * picture.width as usize + source(x, picture.width);
            let to = usize::from(y) * usize::from(surface.width) + usize::from(x);
            if let (Some(pixel), Some(slot)) =
                (picture.pixels.get(from), surface.pixels.get_mut(to))
            {
                *slot = *pixel;
            }
        }
    }
    Some(surface)
}

#[cfg(test)]
#[path = "image_tests.rs"]
mod tests;
