#![allow(clippy::too_many_arguments)]

use std::sync::Mutex;

use crate::border::QuadElement;
use crate::draw::pointer::PointerElement;
use slog::Logger;
use slog_scope::warn;
use smithay::backend::renderer::gles2::{Gles2Renderbuffer, Gles2Renderer};
use smithay::backend::renderer::multigpu::egl::EglGlesBackend;
use smithay::backend::renderer::multigpu::{Error as MultiError, MultiFrame, MultiRenderer};

use smithay::desktop::space::{RenderElement, SpaceOutputTuple, SurfaceTree};
use smithay::input::pointer::CursorImageAttributes;
use smithay::reexports::wayland_server::protocol::wl_surface;
use smithay::utils::{Logical, Physical, Point, Rectangle, Scale};
use smithay::wayland::compositor;

pub type GlMultiRenderer<'a> =
    MultiRenderer<'a, 'a, EglGlesBackend, EglGlesBackend, Gles2Renderbuffer>;
pub type GlMultiFrame = MultiFrame<EglGlesBackend, EglGlesBackend>;

pub const TILING_Z_INDEX: u8 = 100;
pub const BORDER_Z_INDEX: u8 = 102;
pub const POP_UP_Z_INDEX: u8 = 103;
pub const FLOATING_Z_INDEX: u8 = 104;
pub const CURSOR_Z_INDEX: u8 = 255;

smithay::custom_elements! {
    pub CustomElem<=Gles2Renderer>;
    Quad=QuadElement,
    SurfaceTree=SurfaceTree,
    PointerElement=PointerElement,
}

// From cosmic-comp https://github.com/pop-os/cosmic-comp/blob/master_jammy/src/backend/render/mod.rs#L42-L126
impl RenderElement<GlMultiRenderer<'_>> for CustomElem {
    fn id(&self) -> usize {
        RenderElement::<Gles2Renderer>::id(self)
    }

    fn location(&self, scale: impl Into<Scale<f64>>) -> Point<f64, Physical> {
        RenderElement::<Gles2Renderer>::location(self, scale)
    }

    fn geometry(&self, scale: impl Into<Scale<f64>>) -> Rectangle<i32, Physical> {
        RenderElement::<Gles2Renderer>::geometry(self, scale)
    }

    fn accumulated_damage(
        &self,
        scale: impl Into<Scale<f64>>,
        for_values: Option<SpaceOutputTuple<'_, '_>>,
    ) -> Vec<Rectangle<i32, Physical>> {
        RenderElement::<Gles2Renderer>::accumulated_damage(self, scale, for_values)
    }

    fn opaque_regions(
        &self,
        scale: impl Into<Scale<f64>>,
    ) -> Option<Vec<Rectangle<i32, Physical>>> {
        RenderElement::<Gles2Renderer>::opaque_regions(self, scale)
    }

    fn draw(
        &self,
        renderer: &mut GlMultiRenderer<'_>,
        frame: &mut GlMultiFrame,
        scale: impl Into<Scale<f64>>,
        location: Point<f64, Physical>,
        damage: &[Rectangle<i32, Physical>],
        log: &Logger,
    ) -> Result<(), MultiError<EglGlesBackend, EglGlesBackend>> {
        RenderElement::<Gles2Renderer>::draw(
            self,
            renderer.as_mut(),
            frame.as_mut(),
            scale,
            location,
            damage,
            log,
        )
        .map_err(MultiError::Render)
    }

    fn z_index(&self) -> u8 {
        RenderElement::<Gles2Renderer>::z_index(self)
    }
}

pub trait AsGles2Renderer {
    fn as_gles2(&mut self) -> &mut Gles2Renderer;
}

impl AsGles2Renderer for Gles2Renderer {
    fn as_gles2(&mut self) -> &mut Gles2Renderer {
        self
    }
}

impl AsGles2Renderer for GlMultiRenderer<'_> {
    fn as_gles2(&mut self) -> &mut Gles2Renderer {
        self.as_mut()
    }
}

pub fn draw_cursor(
    surface: wl_surface::WlSurface,
    location: impl Into<Point<i32, Logical>>,
) -> SurfaceTree {
    let mut position = location.into();
    let ret = compositor::with_states(&surface, |states| {
        Some(
            states
                .data_map
                .get::<Mutex<CursorImageAttributes>>()
                .unwrap()
                .lock()
                .unwrap()
                .hotspot,
        )
    });

    position -= match ret {
        Some(h) => h,
        None => {
            warn!(
                "Trying to display as a cursor a surface that does not have the CursorImage role."
            );
            (0, 0).into()
        }
    };
    SurfaceTree {
        surface,
        position,
        z_index: CURSOR_Z_INDEX,
    }
}

pub fn draw_dnd_icon(surface: wl_surface::WlSurface, position: Point<i32, Logical>) -> SurfaceTree {
    if compositor::get_role(&surface) != Some("dnd_icon") {
        warn!("Trying to display as a dnd icon a surface that does not have the DndIcon role.");
    }
    SurfaceTree {
        surface,
        position,
        z_index: CURSOR_Z_INDEX,
    }
}
