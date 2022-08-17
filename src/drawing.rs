#![allow(clippy::too_many_arguments)]

use std::sync::Mutex;

use crate::border::QuadElement;
use slog::Logger;
use smithay::backend::renderer::gles2::{Gles2Renderbuffer, Gles2Renderer, Gles2Texture};
use smithay::backend::renderer::multigpu::egl::EglGlesBackend;
use smithay::backend::renderer::multigpu::{Error as MultiError, MultiFrame, MultiRenderer};
use smithay::backend::renderer::{Frame, ImportAll, Renderer, Texture};
use smithay::desktop::space::{RenderElement, SpaceOutputTuple, SurfaceTree};
use smithay::reexports::wayland_server::protocol::wl_surface;
use smithay::utils::{Logical, Physical, Point, Rectangle, Scale, Size, Transform};
use smithay::wayland::compositor::{get_role, with_states};
use smithay::wayland::seat::CursorImageAttributes;

pub static CLEAR_COLOR: [f32; 4] = [0.8, 0.8, 0.9, 1.0];

pub type GlMultiRenderer<'a> =
    MultiRenderer<'a, 'a, EglGlesBackend, EglGlesBackend, Gles2Renderbuffer>;
pub type GlMultiFrame = MultiFrame<EglGlesBackend, EglGlesBackend>;

smithay::custom_elements! {
    pub CustomElem<=Gles2Renderer>;
    Quad=QuadElement,
    SurfaceTree=SurfaceTree,
    PointerElement=PointerElement::<Gles2Texture>,
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
    _log: &Logger,
) -> SurfaceTree {
    let mut position = location.into();
    position -= with_states(&surface, |states| {
        states
            .data_map
            .get::<Mutex<CursorImageAttributes>>()
            .unwrap()
            .lock()
            .unwrap()
            .hotspot
    });
    SurfaceTree {
        surface,
        position,
        z_index: 100,
        /* Cursor should always be on-top */
    }
}

pub fn draw_dnd_icon(
    surface: wl_surface::WlSurface,
    location: impl Into<Point<i32, Logical>>,
    log: &Logger,
) -> SurfaceTree {
    if get_role(&surface) != Some("dnd_icon") {
        slog::warn!(
            log,
            "Trying to display as a dnd icon a surface that does not have the DndIcon role."
        );
    }
    SurfaceTree {
        surface,
        position: location.into(),
        z_index: 100,
        /* Cursor should always be on-top */
    }
}

pub struct PointerElement<T: Texture> {
    texture: T,
    position: Point<i32, Logical>,
    size: Size<i32, Logical>,
}

impl<T: Texture> PointerElement<T> {
    pub fn new(texture: T, pointer_pos: Point<i32, Logical>) -> PointerElement<T> {
        let size = texture.size().to_logical(1, Transform::Normal);
        PointerElement {
            texture,
            position: pointer_pos,
            size,
        }
    }
}

impl<R> RenderElement<R> for PointerElement<<R as Renderer>::TextureId>
where
    R: Renderer + ImportAll,
    <R as Renderer>::TextureId: 'static,
{
    fn id(&self) -> usize {
        0
    }

    fn location(&self, scale: impl Into<Scale<f64>>) -> Point<f64, Physical> {
        self.position.to_f64().to_physical(scale)
    }

    fn geometry(&self, scale: impl Into<Scale<f64>>) -> Rectangle<i32, Physical> {
        Rectangle::from_loc_and_size(self.position, self.size).to_physical_precise_round(scale)
    }

    fn accumulated_damage(
        &self,
        scale: impl Into<Scale<f64>>,
        _: Option<SpaceOutputTuple<'_, '_>>,
    ) -> Vec<Rectangle<i32, Physical>> {
        let scale = scale.into();
        vec![Rectangle::from_loc_and_size(self.position, self.size).to_physical_precise_up(scale)]
    }

    fn opaque_regions(
        &self,
        _scale: impl Into<Scale<f64>>,
    ) -> Option<Vec<Rectangle<i32, Physical>>> {
        let _point = self.position;
        None
    }

    fn draw(
        &self,
        _renderer: &mut R,
        frame: &mut <R as Renderer>::Frame,
        scale: impl Into<Scale<f64>>,
        location: Point<f64, Physical>,
        _damage: &[Rectangle<i32, Physical>],
        _log: &Logger,
    ) -> Result<(), <R as Renderer>::Error> {
        let scale = scale.into();
        frame.render_texture_at(
            &self.texture,
            location.to_i32_round(),
            1,
            scale,
            Transform::Normal,
            &[Rectangle::from_loc_and_size(
                (0, 0),
                self.size.to_physical_precise_round(scale),
            )],
            1.0,
        )?;
        Ok(())
    }
}