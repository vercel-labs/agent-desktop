//! One window of one cursor. It uploads pixels only when what it shows
//! changes, and moves, maps and unmaps itself as the scene asks.
use agent_desktop_core::AdapterError;
use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::shape::{ConnectionExt as _, SK, SO};
use x11rb::protocol::xproto::*;

use super::art::Art;
use super::piece::Piece;
use super::raster::Surface;
use super::stage::Stage;
use super::text::LabelFont;
use crate::system::overlay::cursor_overlay::error;

const PUT_IMAGE_HEADER: usize = 24;
const HALF_OPAQUE: u8 = 128;

/// What a window last uploaded: the art, its opacity out of 255, and whether
/// it was drawn for a compositing manager.
type Drawn = (Art, u8, bool);

pub(crate) struct SlotWindow {
    window: Window,
    mapped: bool,
    origin: (i32, i32),
    size: (u16, u16),
    drawn: Option<Drawn>,
}

impl SlotWindow {
    pub(crate) fn create(stage: &Stage, owner: &[u8]) -> Result<Self, AdapterError> {
        Ok(Self {
            window: stage.create_window(owner)?,
            mapped: false,
            origin: (0, 0),
            size: (1, 1),
            drawn: None,
        })
    }

    pub(crate) fn is_mapped(&self) -> bool {
        self.mapped
    }

    /// Shows `piece`. Without a compositing manager there is no alpha to
    /// blend, so a part under half opacity is not shown and the rest is
    /// drawn solid inside a shape cut from its pixels.
    pub(crate) fn show(
        &mut self,
        stage: &Stage,
        piece: &Piece,
        font: Option<&LabelFont>,
        composited: bool,
    ) -> Result<bool, AdapterError> {
        let level = (piece.opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
        if !composited && level < HALF_OPAQUE {
            self.hide(stage)?;
            return Ok(false);
        }
        let drawn = (piece.art.clone(), level, composited);
        if self.drawn.as_ref() != Some(&drawn) {
            let Some(mut surface) = piece.art.render(font) else {
                self.hide(stage)?;
                return Ok(false);
            };
            if !composited {
                surface = surface.solidified();
            } else if level < u8::MAX {
                surface.dim(f64::from(level) / 255.0);
            }
            self.upload(stage, &surface, composited)?;
            self.drawn = Some(drawn);
        }
        if piece.origin != self.origin {
            self.origin = piece.origin;
            stage
                .connection
                .configure_window(
                    self.window,
                    &ConfigureWindowAux::new()
                        .x(piece.origin.0)
                        .y(piece.origin.1),
                )
                .map_err(error)?;
        }
        let newly_mapped = !self.mapped;
        if newly_mapped {
            stage.connection.map_window(self.window).map_err(error)?;
            self.mapped = true;
        }
        Ok(newly_mapped)
    }

    fn upload(
        &mut self,
        stage: &Stage,
        surface: &Surface,
        composited: bool,
    ) -> Result<(), AdapterError> {
        let connection = &stage.connection;
        let (width, height) = (surface.width, surface.height);
        if (width, height) != self.size {
            self.size = (width, height);
            connection
                .configure_window(
                    self.window,
                    &ConfigureWindowAux::new()
                        .width(u32::from(width))
                        .height(u32::from(height)),
                )
                .map_err(error)?;
        }
        let pixmap = connection.generate_id().map_err(error)?;
        connection
            .create_pixmap(32, pixmap, self.window, width, height)
            .map_err(error)?;
        let gc = connection.generate_id().map_err(error)?;
        connection
            .create_gc(gc, pixmap, &CreateGCAux::new())
            .map_err(error)?;
        let most_significant_first = connection.setup().image_byte_order == ImageOrder::MSB_FIRST;
        let bytes = surface.to_bytes(most_significant_first);
        let row = usize::from(width) * 4;
        let band = ((connection
            .maximum_request_bytes()
            .saturating_sub(PUT_IMAGE_HEADER))
            / row)
            .max(1);
        for (index, rows) in bytes.chunks(row * band).enumerate() {
            let top = (index * band) as i16;
            let count = (rows.len() / row) as u16;
            connection
                .put_image(
                    ImageFormat::Z_PIXMAP,
                    pixmap,
                    gc,
                    width,
                    count,
                    0,
                    top,
                    0,
                    32,
                    rows,
                )
                .map_err(error)?;
        }
        connection
            .change_window_attributes(
                self.window,
                &ChangeWindowAttributesAux::new().background_pixmap(pixmap),
            )
            .map_err(error)?;
        connection.free_gc(gc).map_err(error)?;
        connection.free_pixmap(pixmap).map_err(error)?;
        if composited {
            connection
                .shape_mask(SO::SET, SK::BOUNDING, self.window, 0, 0, x11rb::NONE)
                .map_err(error)?;
        } else {
            let runs: Vec<Rectangle> = surface
                .opaque_runs()
                .into_iter()
                .map(|(x, y, width)| Rectangle {
                    x,
                    y,
                    width,
                    height: 1,
                })
                .collect();
            connection
                .shape_rectangles(
                    SO::SET,
                    SK::BOUNDING,
                    ClipOrdering::YX_BANDED,
                    self.window,
                    0,
                    0,
                    &runs,
                )
                .map_err(error)?;
        }
        connection
            .clear_area(false, self.window, 0, 0, 0, 0)
            .map_err(error)?;
        Ok(())
    }

    pub(crate) fn raise(&self, stage: &Stage) -> Result<(), AdapterError> {
        stage
            .connection
            .configure_window(
                self.window,
                &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE),
            )
            .map_err(error)?;
        Ok(())
    }

    pub(crate) fn hide(&mut self, stage: &Stage) -> Result<(), AdapterError> {
        if self.mapped {
            stage.connection.unmap_window(self.window).map_err(error)?;
            self.mapped = false;
        }
        Ok(())
    }

    pub(crate) fn destroy(self, stage: &Stage) -> Result<(), AdapterError> {
        stage
            .connection
            .destroy_window(self.window)
            .map_err(error)?;
        Ok(())
    }
}
